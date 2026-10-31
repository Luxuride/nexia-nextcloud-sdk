use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use nexia_nextcloud_core::ServerUrl;
use rusqlite::{Connection, OpenFlags};

use crate::account::KAccount;
use crate::error::KAccountsError;
use crate::paths::find_active_accounts_db;

/// Which table layout the opened database uses.
///
/// KDE Plasma's `accounts.db` is created by libaccounts-glib, which keeps account settings in
/// `Settings(account, service, key, type, value)` and its services in `Services(id, …)`. Earlier
/// SDK builds and their fixtures used the flat `AccountSettings`/`AccountServices` pair instead.
/// Reading the layout from the database rather than assuming one keeps both readable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AccountsLayout {
    /// `Settings` + `Services`, as created by libaccounts-glib.
    LibAccounts,
    /// `AccountSettings` + `AccountServices`, the flat layout.
    Flat,
}

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
        let layout = self.detect_layout()?;

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

            let settings = self.get_account_settings(layout, id)?;
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

            let services = self.get_account_services(layout, id)?;

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

    /// Reads which layout the database uses from its table names.
    fn detect_layout(&self) -> Result<AccountsLayout, KAccountsError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .map_err(|err| KAccountsError::database("<schema>", err))?;

        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| KAccountsError::database("<schema>", err))?;

        let mut tables = BTreeSet::new();
        for row in rows {
            tables.insert(row.map_err(|err| KAccountsError::database("<schema_row>", err))?);
        }

        // libaccounts-glib is checked first: a database that has both layouts is one the daemon
        // created, and its own tables are the authoritative ones.
        if tables.contains("Settings") && tables.contains("Services") {
            return Ok(AccountsLayout::LibAccounts);
        }
        if tables.contains("AccountSettings") && tables.contains("AccountServices") {
            return Ok(AccountsLayout::Flat);
        }

        Err(KAccountsError::UnsupportedSchema {
            tables: tables.into_iter().collect(),
        })
    }

    fn get_account_settings(
        &self,
        layout: AccountsLayout,
        account_id: u32,
    ) -> Result<HashMap<String, String>, KAccountsError> {
        let (sql, note) = match layout {
            // Account-wide settings carry no service; the account's own value wins over a
            // service-scoped copy of the same key, which is why those rows are read first.
            AccountsLayout::LibAccounts => (
                "SELECT key, value FROM Settings WHERE account = ?1 \
                 ORDER BY service IS NULL DESC, key ASC",
                "<settings>",
            ),
            AccountsLayout::Flat => (
                "SELECT key, value FROM AccountSettings WHERE accountId = ?1",
                "<settings>",
            ),
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| KAccountsError::database(note, err))?;

        let rows = stmt
            .query_map([account_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|err| KAccountsError::database(note, err))?;

        let mut map = HashMap::new();
        for r in rows {
            let (k, v) = r.map_err(|err| KAccountsError::database("<settings_row>", err))?;
            map.entry(k).or_insert(v);
        }
        Ok(map)
    }

    fn get_account_services(
        &self,
        layout: AccountsLayout,
        account_id: u32,
    ) -> Result<Vec<String>, KAccountsError> {
        let (sql, note) = match layout {
            // libaccounts-glib records a service by giving it settings rows, so the settings table
            // is where an account's enabled services are listed.
            AccountsLayout::LibAccounts => (
                "SELECT DISTINCT service FROM Settings \
                 WHERE account = ?1 AND service IS NOT NULL ORDER BY service ASC",
                "<services>",
            ),
            AccountsLayout::Flat => (
                "SELECT service FROM AccountServices WHERE accountId = ?1 AND enabled = 1 \
                 ORDER BY service ASC",
                "<services>",
            ),
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|err| KAccountsError::database(note, err))?;

        let rows = stmt
            .query_map([account_id], |row| row.get::<_, String>(0))
            .map_err(|err| KAccountsError::database(note, err))?;

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

    /// Database as created by libaccounts-glib, the schema KDE Plasma actually ships.
    fn create_libaccounts_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE Accounts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                provider TEXT,
                enabled INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE Services (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE Settings (
                account INTEGER NOT NULL,
                service TEXT,
                key TEXT NOT NULL,
                type TEXT NOT NULL,
                value TEXT
            );

            INSERT INTO Accounts (id, name, provider, enabled) VALUES
                (1, 'Personal Cloud', 'nextcloud', 1),
                (2, 'Work Nextcloud', 'nextcloud', 0),
                (3, 'Google Account', 'google', 1);

            INSERT INTO Services (id, name) VALUES
                ('nextcloud-storage', 'Files'),
                ('nextcloud-caldav', 'Calendar');

            INSERT INTO Settings (account, service, key, type, value) VALUES
                (1, NULL, 'host', 'string', 'https://cloud.example.com'),
                (1, NULL, 'username', 'string', 'alice'),
                (1, 'nextcloud-storage', 'webdav-path', 'string', '/remote.php/dav'),
                (1, 'nextcloud-caldav', 'enabled', 'bool', '1'),
                (2, NULL, 'host', 'string', 'https://neurocloud.luxuride.eu'),
                (2, NULL, 'username', 'string', 'bob'),
                (2, 'nextcloud-storage', 'webdav-path', 'string', '/remote.php/dav'),
                (3, NULL, 'host', 'string', 'https://accounts.google.com'),
                (3, NULL, 'username', 'string', 'carol');
            "#,
        )
        .unwrap();
        conn
    }

    /// Flat layout used by earlier SDK builds and their fixtures.
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

    fn assert_personal_and_work(accounts: &[KAccount]) {
        assert_eq!(accounts.len(), 2, "ownCloud/nextcloud accounts only");

        let personal = &accounts[0];
        assert_eq!(personal.id, 1);
        assert_eq!(personal.name, "Personal Cloud");
        assert_eq!(personal.username, "alice");
        assert_eq!(personal.server_url.as_str(), "https://cloud.example.com");
        assert!(personal.enabled);
        assert_eq!(
            personal.services,
            vec![
                "nextcloud-caldav".to_string(),
                "nextcloud-storage".to_string()
            ]
        );

        let work = &accounts[1];
        assert_eq!(work.id, 2);
        assert_eq!(work.username, "bob");
        assert!(!work.enabled);
        assert_eq!(work.services, vec!["nextcloud-storage".to_string()]);
    }

    #[test]
    fn reads_the_libaccounts_schema() {
        let db = KAccountsDatabase::from_connection(create_libaccounts_db());
        let accounts = db.list_accounts().expect("query succeeds");

        assert_personal_and_work(&accounts);
    }

    #[test]
    fn reads_the_flat_schema() {
        let db = KAccountsDatabase::from_connection(create_test_db());
        let accounts = db.list_accounts().expect("query succeeds");

        assert_personal_and_work(&accounts);
    }

    #[test]
    fn account_wide_settings_win_over_service_scoped_copies() {
        let conn = create_libaccounts_db();
        conn.execute_batch(
            r#"
            INSERT INTO Settings (account, service, key, type, value) VALUES
                (1, 'nextcloud-storage', 'host', 'string', 'https://stale.example.com');
            "#,
        )
        .unwrap();

        let db = KAccountsDatabase::from_connection(conn);
        let accounts = db.list_accounts().expect("query succeeds");

        assert_eq!(
            accounts[0].server_url.as_str(),
            "https://cloud.example.com",
            "the account-wide host must win over a service-scoped duplicate"
        );
    }

    #[test]
    fn an_unknown_schema_names_the_tables_it_found() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE Something (id INTEGER PRIMARY KEY);")
            .unwrap();

        let db = KAccountsDatabase::from_connection(conn);
        let error = db.list_accounts().expect_err("must not guess a schema");

        assert!(
            error.to_string().contains("Something"),
            "the error should list the tables it saw: {error}"
        );
    }

    #[test]
    fn accounts_without_a_url_or_username_are_skipped() {
        let conn = create_libaccounts_db();
        conn.execute_batch(
            r#"
            INSERT INTO Accounts (id, name, provider, enabled) VALUES
                (4, 'Incomplete', 'nextcloud', 1);
            INSERT INTO Settings (account, service, key, type, value) VALUES
                (4, NULL, 'host', 'string', 'not a url');
            "#,
        )
        .unwrap();

        let db = KAccountsDatabase::from_connection(conn);
        let accounts = db.list_accounts().expect("query succeeds");

        // Account 4 has a host but no user name, so it is skipped instead of failing the listing.
        assert_eq!(accounts.len(), 2);
    }

    #[test]
    fn a_malformed_url_is_reported_with_the_value() {
        let conn = create_libaccounts_db();
        conn.execute_batch(
            r#"
            INSERT INTO Accounts (id, name, provider, enabled) VALUES
                (5, 'Broken', 'nextcloud', 1);
            INSERT INTO Settings (account, service, key, type, value) VALUES
                (5, NULL, 'host', 'string', 'ftp://files.example.com'),
                (5, NULL, 'username', 'string', 'dave');
            "#,
        )
        .unwrap();

        let db = KAccountsDatabase::from_connection(conn);
        let error = db.list_accounts().expect_err("a bad URL is an error");

        assert!(
            error.to_string().contains("ftp://files.example.com"),
            "the error should quote the value: {error}"
        );
    }
}
