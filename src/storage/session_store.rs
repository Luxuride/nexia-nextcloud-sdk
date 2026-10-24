use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nextcloud_auth::SecretString;
use nextcloud_sdk_core::ServerUrl;
use nextcloud_sdk_session::{Account, LoginFlow, PollToken};
use nextcloud_sdk_storage::{EncryptedDatabase, StorageError, StoreKey};

fn query_err(operation: &'static str, source: rusqlite::Error) -> StorageError {
    StorageError::Query {
        operation,
        source: Box::new(source),
    }
}

/// Encrypted storage adapter for session credentials and login flows.
pub struct SessionStore {
    database: EncryptedDatabase,
}

impl SessionStore {
    /// Opens the encrypted credentials database at `path` using `key`.
    pub fn open(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let database = EncryptedDatabase::open_credential_store(path, key)?;
        Ok(Self { database })
    }

    /// Borrows the underlying encrypted database.
    pub const fn database(&self) -> &EncryptedDatabase {
        &self.database
    }

    /// Persists an account record, replacing any existing entry for the same server and login name.
    pub fn store_account(&self, account: &Account) -> Result<(), StorageError> {
        tracing::debug!(server = %account.server(), user_id = %account.user_id(), "Storing account in session store");
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.database
            .connection()
            .execute(
                "INSERT INTO account (server, login_name, user_id, app_password, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(server, login_name) DO UPDATE SET
                     user_id = excluded.user_id,
                     app_password = excluded.app_password,
                     created_at = excluded.created_at",
                rusqlite::params![
                    account.server().as_str(),
                    account.login_name(),
                    account.user_id(),
                    account.password().expose().as_bytes(),
                    created_at,
                ],
            )
            .map_err(|e| query_err("storing account credentials", e))?;

        Ok(())
    }

    /// Loads the most recent account record for `server`, if present.
    pub fn load_account(&self, server: &ServerUrl) -> Result<Option<Account>, StorageError> {
        tracing::trace!(server = %server, "Loading account from session store");
        let conn = self.database.connection();
        let mut stmt = conn
            .prepare(
                "SELECT server, login_name, user_id, app_password
                 FROM account
                 WHERE server = ?1
                 ORDER BY id DESC
                 LIMIT 1",
            )
            .map_err(|e| query_err("preparing load account query", e))?;

        let mut rows = stmt
            .query(rusqlite::params![server.as_str()])
            .map_err(|e| query_err("querying account", e))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading account row", e))?
        {
            let server_str: String = row
                .get(0)
                .map_err(|e| query_err("reading server column", e))?;
            let login_name: String = row
                .get(1)
                .map_err(|e| query_err("reading login_name column", e))?;
            let user_id: String = row
                .get(2)
                .map_err(|e| query_err("reading user_id column", e))?;
            let pw_blob: Vec<u8> = row
                .get(3)
                .map_err(|e| query_err("reading app_password column", e))?;

            let pw_str = String::from_utf8(pw_blob).map_err(|e| StorageError::Query {
                operation: "decoding stored app password",
                source: Box::new(e),
            })?;
            let server_url = ServerUrl::parse(&server_str).map_err(|e| StorageError::Query {
                operation: "parsing stored server url",
                source: Box::new(e),
            })?;

            Ok(Some(Account::new(
                server_url,
                login_name,
                user_id,
                SecretString::new(pw_str),
            )))
        } else {
            Ok(None)
        }
    }

    /// Deletes all stored accounts for `server`.
    pub fn delete_account(&self, server: &ServerUrl) -> Result<(), StorageError> {
        tracing::info!(server = %server, "Deleting stored account from session store");
        self.database
            .connection()
            .execute(
                "DELETE FROM account WHERE server = ?1",
                rusqlite::params![server.as_str()],
            )
            .map_err(|e| query_err("deleting account", e))?;

        Ok(())
    }

    /// Stores an in-flight login flow for `server`.
    pub fn store_login_flow(
        &self,
        server: &ServerUrl,
        flow: &LoginFlow,
    ) -> Result<(), StorageError> {
        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.database.connection();
        conn.execute(
            "DELETE FROM login_flow WHERE server = ?1",
            rusqlite::params![server.as_str()],
        )
        .map_err(|e| query_err("clearing previous login flow", e))?;

        conn.execute(
            "INSERT INTO login_flow (server, poll_endpoint, poll_token, login_url, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                server.as_str(),
                flow.poll_endpoint(),
                flow.poll_token().expose().as_bytes(),
                flow.login_url(),
                created_at,
            ],
        )
        .map_err(|e| query_err("storing login flow", e))?;

        Ok(())
    }

    /// Loads any in-flight login flow for `server`.
    pub fn load_login_flow(&self, server: &ServerUrl) -> Result<Option<LoginFlow>, StorageError> {
        let conn = self.database.connection();
        let mut stmt = conn
            .prepare(
                "SELECT poll_endpoint, poll_token, login_url
                 FROM login_flow
                 WHERE server = ?1
                 ORDER BY id DESC
                 LIMIT 1",
            )
            .map_err(|e| query_err("preparing load login flow query", e))?;

        let mut rows = stmt
            .query(rusqlite::params![server.as_str()])
            .map_err(|e| query_err("querying login flow", e))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading login flow row", e))?
        {
            let poll_endpoint: String = row
                .get(0)
                .map_err(|e| query_err("reading poll_endpoint", e))?;
            let token_blob: Vec<u8> = row.get(1).map_err(|e| query_err("reading poll_token", e))?;
            let login_url: String = row.get(2).map_err(|e| query_err("reading login_url", e))?;

            let token_str = String::from_utf8(token_blob).map_err(|e| StorageError::Query {
                operation: "decoding stored poll token",
                source: Box::new(e),
            })?;

            Ok(Some(LoginFlow::new(
                PollToken::new(token_str),
                login_url,
                poll_endpoint,
            )))
        } else {
            Ok(None)
        }
    }

    /// Deletes any pending login flow for `server`.
    pub fn delete_login_flow(&self, server: &ServerUrl) -> Result<(), StorageError> {
        self.database
            .connection()
            .execute(
                "DELETE FROM login_flow WHERE server = ?1",
                rusqlite::params![server.as_str()],
            )
            .map_err(|e| query_err("deleting login flow", e))?;

        Ok(())
    }
}

impl fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionStore")
            .field("path", &self.database.path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "session-store-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn round_trip_account_persistence() {
        let path = temp_path("account");
        let key = StoreKey::generate().unwrap();
        let store = SessionStore::open(&path, &key).unwrap();
        let server = ServerUrl::parse("https://cloud.example.org").unwrap();

        assert!(store.load_account(&server).unwrap().is_none());

        let account = Account::new(
            server.clone(),
            "alice",
            "alice_uid",
            SecretString::new("super-secret-pw"),
        );

        store.store_account(&account).unwrap();

        let loaded = store
            .load_account(&server)
            .unwrap()
            .expect("account exists");
        assert_eq!(loaded.server(), &server);
        assert_eq!(loaded.login_name(), "alice");
        assert_eq!(loaded.user_id(), "alice_uid");
        assert_eq!(loaded.password().expose(), "super-secret-pw");

        // Updating with new password and user_id
        let updated = Account::new(
            server.clone(),
            "alice",
            "alice_uid_updated",
            SecretString::new("new-password"),
        );
        store.store_account(&updated).unwrap();

        let loaded2 = store.load_account(&server).unwrap().unwrap();
        assert_eq!(loaded2.user_id(), "alice_uid_updated");
        assert_eq!(loaded2.password().expose(), "new-password");

        // Deleting
        store.delete_account(&server).unwrap();
        assert!(store.load_account(&server).unwrap().is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn round_trip_login_flow_persistence() {
        let path = temp_path("flow");
        let key = StoreKey::generate().unwrap();
        let store = SessionStore::open(&path, &key).unwrap();
        let server = ServerUrl::parse("https://cloud.example.org").unwrap();

        assert!(store.load_login_flow(&server).unwrap().is_none());

        let flow = LoginFlow::new(
            PollToken::new("poll_tok_123"),
            "https://cloud.example.org/login/v2/flow/xyz",
            "https://cloud.example.org/login/v2/poll",
        );

        store.store_login_flow(&server, &flow).unwrap();

        let loaded = store
            .load_login_flow(&server)
            .unwrap()
            .expect("flow exists");
        assert_eq!(loaded.poll_token().expose(), "poll_tok_123");
        assert_eq!(
            loaded.login_url(),
            "https://cloud.example.org/login/v2/flow/xyz"
        );
        assert_eq!(
            loaded.poll_endpoint(),
            "https://cloud.example.org/login/v2/poll"
        );

        store.delete_login_flow(&server).unwrap();
        assert!(store.load_login_flow(&server).unwrap().is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn debug_redacts_credentials() {
        let path = temp_path("debug");
        let key = StoreKey::generate().unwrap();
        let store = SessionStore::open(&path, &key).unwrap();
        let rendered = format!("{store:?}");
        assert!(rendered.contains("SessionStore"));
        assert!(!rendered.contains("password"));

        std::fs::remove_file(&path).ok();
    }
}
