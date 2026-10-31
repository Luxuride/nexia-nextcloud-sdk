use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::key::StoreKey;
use crate::schema::{CacheSchema, ConfigSchema, CredentialSchema};

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
            let created = !parent.exists();
            std::fs::create_dir_all(parent)
                .map_err(|source| StorageError::path("creating the database directory", source))?;
            // Only tighten a directory this call created: an existing one belongs
            // to the user's setup and refusing to open the database because its
            // mode is wider than expected would break working installs.
            if created && let Err(error) = set_owner_only(parent, 0o700) {
                tracing::warn!(
                    path = %parent.display(),
                    %error,
                    "could not restrict the database directory to the owner"
                );
            }
        }

        let connection = Connection::open(&path).map_err(|source| {
            tracing::error!(path = %path.display(), error = %source, "Failed to open sqlite database file");
            StorageError::open(&path, source)
        })?;
        // The file holds credential ciphertext: a world-readable one can be
        // copied and attacked offline. Failure is reported, not fatal, so a
        // database on a filesystem without mode bits still opens.
        if let Err(error) = set_owner_only(&path, 0o600) {
            tracing::warn!(
                path = %path.display(),
                %error,
                "could not restrict the database file to the owner"
            );
        }

        // `PRAGMA key` first, then keep every temporary table in memory: a sort
        // or `ORDER BY` over cached rows would otherwise spill plaintext
        // credential rows into an unencrypted temp file.
        let key_sql = key.pragma();
        let pragmas = format!("{}\nPRAGMA temp_store = MEMORY;", key_sql.as_str());
        connection.execute_batch(&pragmas).map_err(|source| {
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

    /// Opens the configuration database and installs its schema.
    pub fn open_config_store(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let database = Self::open(path, key)?;
        database.apply_schema(ConfigSchema::CREATE, ConfigSchema::VERSION)?;
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

/// Restricts `path` to the owner only.
///
/// The database holds credential ciphertext, so a world-readable file would let
/// any local user copy it and attack the key offline instead of having to read
/// the plaintext here.
#[cfg(unix)]
fn set_owner_only(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

/// Platforms without Unix mode bits have no equivalent; access is governed by
/// the ACLs of the parent directory.
#[cfg(not(unix))]
fn set_owner_only(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
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
        assert!(!format!("{database:?}").contains(key.to_hex().as_str()));
        std::fs::remove_file(&path).ok();
    }
}

#[cfg(all(test, unix))]
mod permission_tests {
    use std::os::unix::fs::PermissionsExt;

    use super::EncryptedDatabase;
    use crate::StoreKey;

    #[test]
    fn the_database_and_its_directory_are_private() {
        let root = std::env::temp_dir().join(format!(
            "sdk-storage-perms-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let nested = root.join("nested");
        let path = nested.join("auth.db");
        let key = StoreKey::generate().unwrap();

        drop(EncryptedDatabase::open_credential_store(&path, &key).unwrap());

        let dir_mode = std::fs::metadata(&nested).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "directory mode {dir_mode:o}");
        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "file mode {file_mode:o}");

        std::fs::remove_dir_all(&root).ok();
    }
}
