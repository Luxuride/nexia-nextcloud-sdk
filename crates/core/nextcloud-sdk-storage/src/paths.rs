use std::fmt;
use std::path::{Path, PathBuf};

use crate::database::EncryptedDatabase;
use crate::{StorageError, StoreKey};

/// File locations of the two SDK databases.
///
/// Paths are chosen by the application, not derived from a crate-level
/// constant, so a desktop app can keep them in `$XDG_DATA_HOME` while a
/// portable build keeps them next to its executable.
#[derive(Clone, PartialEq, Eq)]
pub struct StoragePaths {
    auth: PathBuf,
    cache: PathBuf,
}

impl StoragePaths {
    /// Uses exactly these two files.
    pub fn new(auth: impl Into<PathBuf>, cache: impl Into<PathBuf>) -> Self {
        Self {
            auth: auth.into(),
            cache: cache.into(),
        }
    }

    /// Puts both databases in `directory`.
    pub fn in_directory(directory: impl AsRef<Path>) -> Self {
        let directory = directory.as_ref();
        Self::new(directory.join("auth.db"), directory.join("cache.db"))
    }

    /// Defaults to `<data dir>/nextcloud-rust-sdk/{auth,cache}.db`.
    ///
    /// Uses `$XDG_DATA_HOME`, falling back to `$HOME/.local/share`.
    pub fn default_xdg() -> Result<Self, StorageError> {
        let data_home = match std::env::var_os("XDG_DATA_HOME") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => match std::env::var_os("HOME") {
                Some(home) if !home.is_empty() => PathBuf::from(home).join(".local/share"),
                _ => {
                    return Err(StorageError::path(
                        "resolving the user data directory",
                        std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "neither XDG_DATA_HOME nor HOME is set",
                        ),
                    ));
                }
            },
        };
        Ok(Self::in_directory(data_home.join("nextcloud-rust-sdk")))
    }

    /// Path of the credentials database.
    pub fn auth(&self) -> &Path {
        &self.auth
    }

    /// Path of the cache database.
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    /// Opens (creating if needed) the credentials database with `key`.
    pub fn open_auth_db(&self, key: &StoreKey) -> Result<EncryptedDatabase, StorageError> {
        EncryptedDatabase::open_credential_store(&self.auth, key)
    }

    /// Opens (creating if needed) the cache database with `key`.
    pub fn open_cache_db(&self, key: &StoreKey) -> Result<EncryptedDatabase, StorageError> {
        EncryptedDatabase::open_cache_store(&self.cache, key)
    }

    /// Creates both parent directories, so a first run can create the files.
    pub fn ensure_directories(&self) -> Result<(), StorageError> {
        for path in [&self.auth, &self.cache] {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).map_err(|source| {
                    StorageError::path("creating the storage directory", source)
                })?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for StoragePaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoragePaths")
            .field("auth", &self.auth)
            .field("cache", &self.cache)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::StoragePaths;

    #[test]
    fn in_directory_places_both_databases_side_by_side() {
        let paths = StoragePaths::in_directory("/tmp/nc");
        assert_eq!(paths.auth(), std::path::Path::new("/tmp/nc/auth.db"));
        assert_eq!(paths.cache(), std::path::Path::new("/tmp/nc/cache.db"));
    }

    #[test]
    fn explicit_paths_are_kept_verbatim() {
        let paths = StoragePaths::new("/a/auth.sqlite", "/b/cache.sqlite");
        assert_eq!(paths.auth(), std::path::Path::new("/a/auth.sqlite"));
        assert_eq!(paths.cache(), std::path::Path::new("/b/cache.sqlite"));
    }

    #[test]
    fn ensure_directories_creates_the_parent() {
        let root = std::env::temp_dir().join(format!(
            "sdk-storage-paths-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let paths = StoragePaths::in_directory(root.join("nested/deeper"));
        paths.ensure_directories().unwrap();
        assert!(root.join("nested/deeper").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }
}
