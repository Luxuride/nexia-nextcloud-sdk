use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::key::StoreKey;
use crate::schema::{CacheSchema, CredentialSchema};

/// An open SQLCipher database.
///
/// The connection is opened with the key applied before anything else, so a
/// wrong key surfaces as [`StorageError::KeyRejected`] at open time rather
/// than as a confusing read failure later.
pub struct EncryptedDatabase {
    connection: Connection,
    path: PathBuf,
}

impl EncryptedDatabase {
    /// Opens `path`, creating the file if it does not exist, and applies `key`.
    pub fn open(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();
        tracing::debug!(path = %path.display(), "Opening encrypted database");
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|source| StorageError::path("creating the database directory", source))?;
        }

        let connection = Connection::open(&path).map_err(|source| {
            tracing::error!(path = %path.display(), error = %source, "Failed to open sqlite database file");
            StorageError::open(&path, source)
        })?;
        connection.execute_batch(&key.pragma()).map_err(|source| {
            tracing::error!(path = %path.display(), error = %source, "Failed to apply encryption key pragma");
            StorageError::open(&path, source)
        })?;

        // The first read after `PRAGMA key` is what actually validates it.
        connection
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|source| {
                tracing::error!(path = %path.display(), error = %source, "Encryption key rejected by database");
                StorageError::key_rejected(&path, source)
            })?;

        tracing::info!(path = %path.display(), "Encrypted database opened successfully");
        Ok(Self { connection, path })
    }

    /// Opens the credentials database and installs its schema.
    pub fn open_credential_store(
        path: impl AsRef<Path>,
        key: &StoreKey,
    ) -> Result<Self, StorageError> {
        let database = Self::open(path, key)?;
        database.apply_schema(CredentialSchema::CREATE, CredentialSchema::VERSION)?;
        Ok(database)
    }

    /// Opens the cache database and installs its schema.
    pub fn open_cache_store(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let database = Self::open(path, key)?;
        database.apply_schema(CacheSchema::CREATE, CacheSchema::VERSION)?;
        Ok(database)
    }

    /// File this database was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The underlying connection.
    pub const fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Mutable access, for callers that need a transaction.
    pub const fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    /// The schema version recorded in the file.
    pub fn user_version(&self) -> Result<i64, StorageError> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|source| StorageError::query("reading the schema version", source))
    }

    fn apply_schema(&self, statements: &[&str], version: i64) -> Result<(), StorageError> {
        tracing::debug!(version, "Applying database schema");
        for statement in statements {
            self.connection
                .execute_batch(statement)
                .map_err(|source| StorageError::query("creating the schema", source))?;
        }
        self.connection
            .execute_batch(&format!("PRAGMA user_version = {version}"))
            .map_err(|source| StorageError::query("recording the schema version", source))
    }
}

impl fmt::Debug for EncryptedDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptedDatabase")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::EncryptedDatabase;
    use crate::{StorageError, StoreKey};

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "sdk-storage-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn opens_with_the_key_that_created_it() {
        let path = temp_path("reopen");
        let key = StoreKey::generate().unwrap();
        drop(EncryptedDatabase::open_credential_store(&path, &key).unwrap());
        let reopened = EncryptedDatabase::open(&path, &key).unwrap();
        assert!(reopened.path().ends_with(path.file_name().unwrap()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_different_key_is_rejected_at_open() {
        let path = temp_path("wrong-key");
        let key = StoreKey::generate().unwrap();
        drop(EncryptedDatabase::open_credential_store(&path, &key).unwrap());
        let other = StoreKey::generate().unwrap();
        let error = EncryptedDatabase::open(&path, &other).expect_err("wrong key must fail");
        assert!(matches!(error, StorageError::KeyRejected { .. }));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn credential_schema_installs_its_tables_and_version() {
        let path = temp_path("cred-schema");
        let key = StoreKey::generate().unwrap();
        let database = EncryptedDatabase::open_credential_store(&path, &key).unwrap();
        assert_eq!(database.user_version().unwrap(), 1);
        let count: i64 = database
            .connection()
            .query_row("SELECT count(*) FROM account", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn cache_schema_installs_its_tables() {
        let path = temp_path("cache-schema");
        let key = StoreKey::generate().unwrap();
        let database = EncryptedDatabase::open_cache_store(&path, &key).unwrap();
        let count: i64 = database
            .connection()
            .query_row("SELECT count(*) FROM dav_entry", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn opening_creates_missing_parent_directories() {
        let root = temp_path("nested-dir");
        let path = root.join("deeper/store.db");
        let key = StoreKey::generate().unwrap();
        drop(EncryptedDatabase::open(&path, &key).unwrap());
        assert!(path.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn debug_does_not_leak_the_key() {
        let path = temp_path("debug");
        let key = StoreKey::generate().unwrap();
        let database = EncryptedDatabase::open(&path, &key).unwrap();
        assert!(!format!("{database:?}").contains(&key.to_hex()));
        std::fs::remove_file(&path).ok();
    }
}
