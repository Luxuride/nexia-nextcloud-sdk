use std::collections::HashMap;
use std::path::Path;

use nexia_nextcloud_core::ServerUrl;
use rusqlite::{Connection, OpenFlags};

use crate::account::KAccount;
use crate::error::KAccountsError;
use crate::paths::find_active_accounts_db;

/// Read-only reader for the KDE Plasma / Accounts-SSO SQLite database (`accounts.db`).
#[derive(Debug)]
pub struct KAccountsDatabase {
    conn: Connection,
}

impl KAccountsDatabase {
    /// Opens the system KAccounts database from its default location.
    pub fn open_default() -> Result<Self, KAccountsError> {
        let Some(path) = find_active_accounts_db() else {
            return Err(KAccountsError::NotFound {
                searched: crate::paths::discover_accounts_db_paths(),
            });
        };
        Self::open(&path)
    }

    /// Opens the KAccounts database from a specific file path in read-only mode.
    pub fn open(path: &Path) -> Result<Self, KAccountsError> {
        tracing::debug!(path = %path.display(), "Opening KAccounts SQLite database");
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(|err| KAccountsError::database(path, err))?;

        Ok(Self { conn })
    }

    /// Creates a reader backed by an existing SQLite connection, useful for testing.
    pub fn from_connection(conn: Connection) -> Self {
        Self { conn }
    }

    /// Queries all Nextcloud and ownCloud accounts stored in the database.
    pub fn list_accounts(&self) -> Result<Vec<KAccount>, KAccountsError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, provider, enabled FROM Accounts \
                 WHERE provider IN ('nextcloud', 'owncloud') \
                 ORDER BY id ASC",
            )
            .map_err(|err| KAccountsError::database("<query>", err))?;

        let account_rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i32>(3)? != 0,
                ))
            })
            .map_err(|err| KAccountsError::database("<query>", err))?;

        let mut accounts = Vec::new();
        for res in account_rows {
            let (id, name, provider, enabled) =
                res.map_err(|err| KAccountsError::database("<row>", err))?;

            // Retrieve settings
            let settings = self.get_account_settings(id)?;
            let raw_url = settings
                .get("host")
                .or_else(|| settings.get("server"))
                .or_else(|| settings.get("url"))
                .cloned();

            let username = settings
                .get("username")
                .or_else(|| settings.get("user"))
                .cloned();

            let (Some(raw_url), Some(username)) = (raw_url, username) else {
                continue;
            };

            let server_url = ServerUrl::parse(&raw_url)
                .map_err(|err| KAccountsError::invalid_url(raw_url, err))?;

            let services = self.get_account_services(id)?;

            accounts.push(KAccount {
                id,
                name,
                provider,
                server_url,
                username,
                enabled,
                services,
            });
        }

        tracing::info!(count = accounts.len(), "Queried KAccounts accounts");
        Ok(accounts)
    }

    fn get_account_settings(
        &self,
        account_id: u32,
    ) -> Result<HashMap<String, String>, KAccountsError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM AccountSettings WHERE accountId = ?")
            .map_err(|err| KAccountsError::database("<settings>", err))?;

        let rows = stmt
            .query_map([account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| KAccountsError::database("<settings>", err))?;

        let mut map = HashMap::new();
        for r in rows {
            let (k, v) = r.map_err(|err| KAccountsError::database("<settings_row>", err))?;
            map.insert(k, v);
        }
        Ok(map)
    }

    fn get_account_services(&self, account_id: u32) -> Result<Vec<String>, KAccountsError> {
        let mut stmt = self
            .conn
            .prepare("SELECT service FROM AccountServices WHERE accountId = ? AND enabled = 1")
            .map_err(|err| KAccountsError::database("<services>", err))?;

        let rows = stmt
            .query_map([account_id], |row| row.get::<_, String>(0))
            .map_err(|err| KAccountsError::database("<services>", err))?;

        let mut list = Vec::new();
        for r in rows {
            let s = r.map_err(|err| KAccountsError::database("<services_row>", err))?;
            list.push(s);
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE Accounts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT NOT NULL,
                enabled INTEGER NOT NULL
            );
            CREATE TABLE AccountSettings (
                accountId INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE AccountServices (
                accountId INTEGER NOT NULL,
                service TEXT NOT NULL,
                enabled INTEGER NOT NULL
            );

            INSERT INTO Accounts (id, name, provider, enabled) VALUES
                (1, 'Personal Cloud', 'nextcloud', 1),
                (2, 'Work Nextcloud', 'nextcloud', 0),
                (3, 'Google Account', 'google', 1);

            INSERT INTO AccountSettings (accountId, key, value) VALUES
                (1, 'host', 'https://cloud.example.com'),
                (1, 'username', 'alice'),
                (2, 'host', 'https://neurocloud.luxuride.eu'),
                (2, 'username', 'bob');

            INSERT INTO AccountServices (accountId, service, enabled) VALUES
                (1, 'nextcloud-storage', 1),
                (1, 'nextcloud-caldav', 1),
                (1, 'nextcloud-carddav', 0),
                (2, 'nextcloud-storage', 1);
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn queries_accounts_and_filters_non_nextcloud() {
        let db = KAccountsDatabase::from_connection(create_test_db());
        let accounts = db.list_accounts().unwrap();
        assert_eq!(accounts.len(), 2);

        let a1 = &accounts[0];
        assert_eq!(a1.id, 1);
        assert_eq!(a1.name, "Personal Cloud");
        assert_eq!(a1.server_url.as_str(), "https://cloud.example.com");
        assert_eq!(a1.username, "alice");
        assert!(a1.enabled);
        assert_eq!(
            a1.services,
            vec![
                "nextcloud-storage".to_string(),
                "nextcloud-caldav".to_string()
            ]
        );

        let a2 = &accounts[1];
        assert_eq!(a2.id, 2);
        assert_eq!(a2.name, "Work Nextcloud");
        assert_eq!(a2.server_url.as_str(), "https://neurocloud.luxuride.eu");
        assert_eq!(a2.username, "bob");
        assert!(!a2.enabled);
    }
}
