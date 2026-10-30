/// Tables the credentials database is expected to carry.
///
/// The storage crate owns the schema so the crates above never hand-write
/// DDL; a migration step can be added later by bumping [`Self::VERSION`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialSchema;

impl CredentialSchema {
    /// Schema version recorded in `PRAGMA user_version`.
    pub const VERSION: i64 = 1;

    /// DDL creating every table of the credentials database.
    pub const CREATE: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS account (
            id INTEGER PRIMARY KEY,
            server TEXT NOT NULL,
            login_name TEXT NOT NULL,
            user_id TEXT NOT NULL,
            app_password BLOB NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE (server, login_name)
        )",
        "CREATE TABLE IF NOT EXISTS login_flow (
            id INTEGER PRIMARY KEY,
            server TEXT NOT NULL,
            poll_endpoint TEXT NOT NULL,
            poll_token BLOB NOT NULL,
            login_url TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
    ];
}

/// Tables the cache database is expected to carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheSchema;

impl CacheSchema {
    /// Schema version recorded in `PRAGMA user_version`.
    pub const VERSION: i64 = 1;

    /// DDL creating every table of the cache database.
    pub const CREATE: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS capabilities (
            server TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            body BLOB NOT NULL,
            fetched_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS dav_entry (
            server TEXT NOT NULL,
            user_id TEXT NOT NULL,
            path TEXT NOT NULL,
            etag TEXT,
            content_type TEXT,
            size INTEGER,
            modified_at TEXT,
            fetched_at INTEGER NOT NULL,
            PRIMARY KEY (server, user_id, path)
        )",
    ];
}

/// Tables the configuration and runtime database is expected to carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigSchema;

impl ConfigSchema {
    /// Schema version recorded in `PRAGMA user_version`.
    pub const VERSION: i64 = 1;

    /// DDL creating every table of the configuration database.
    pub const CREATE: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS sync_mount (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            remote_path TEXT NOT NULL,
            local_target TEXT NOT NULL,
            mode TEXT NOT NULL,
            display_name TEXT,
            enabled INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS theme_cache (
            server TEXT PRIMARY KEY,
            name TEXT,
            slogan TEXT,
            color TEXT,
            color_text TEXT,
            logo_path TEXT,
            background_url TEXT,
            updated_at INTEGER NOT NULL
        )",
    ];
}
