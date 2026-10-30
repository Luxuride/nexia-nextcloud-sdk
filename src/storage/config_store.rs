use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use nexia_nextcloud_sdk_storage::{EncryptedDatabase, StorageError, StoreKey};
use serde::{Deserialize, Serialize};

fn query_err(operation: &'static str, source: rusqlite::Error) -> StorageError {
    StorageError::Query {
        operation,
        source: Box::new(source),
    }
}

/// Persistent cached visual branding and server metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedTheme {
    /// Server instance authority or URL.
    pub server: String,
    /// Human-visible instance name (e.g. `"Nextcloud"` or custom organization name).
    pub name: Option<String>,
    /// Server instance tagline or slogan.
    pub slogan: Option<String>,
    /// Primary brand hexadecimal color (e.g. `"#0082c9"`).
    pub color: Option<String>,
    /// Contrasting text hexadecimal color (e.g. `"#ffffff"`).
    pub color_text: Option<String>,
    /// Local cached path to the server branding logo.
    pub logo_path: Option<String>,
    /// Optional remote background URL.
    pub background_url: Option<String>,
    /// UNIX timestamp in seconds when the theme was last refreshed.
    pub updated_at: i64,
}

/// A persistent synchronization mount descriptor stored across sessions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSyncMount {
    /// Unique identifier for this sync mount.
    pub id: String,
    /// Associated account identifier.
    pub account_id: String,
    /// Nextcloud remote directory path (e.g. `"/"`, `"/Documents"`).
    pub remote_path: String,
    /// Absolute local filesystem target directory.
    pub local_target: String,
    /// Synchronization mode (`"VirtualVfs"` or `"BidirectionalSync"`).
    pub mode: String,
    /// Optional user-visible display label.
    pub display_name: Option<String>,
    /// Whether automatic synchronization is enabled for this mount.
    pub enabled: bool,
    /// UNIX timestamp when created.
    pub created_at: i64,
    /// UNIX timestamp when last updated.
    pub updated_at: i64,
}

/// Encrypted storage adapter for permanent configuration, settings, themes, and sync mounts.
pub struct ConfigStore {
    database: EncryptedDatabase,
}

impl ConfigStore {
    /// Opens the encrypted configuration database at `path` using `key`.
    pub fn open(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let database = EncryptedDatabase::open_config_store(path, key)?;
        Ok(Self { database })
    }

    /// Borrows the underlying encrypted database.
    pub const fn database(&self) -> &EncryptedDatabase {
        &self.database
    }

    /// Stores or updates an arbitrary key-value setting.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.database
            .connection()
            .execute(
                "INSERT INTO settings (key, value, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     updated_at = excluded.updated_at",
                rusqlite::params![key, value, now],
            )
            .map_err(|e| query_err("storing setting in config store", e))?;

        Ok(())
    }

    /// Retrieves an arbitrary key-value setting.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut stmt = self
            .database
            .connection()
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| query_err("preparing select setting", e))?;

        let mut rows = stmt
            .query(rusqlite::params![key])
            .map_err(|e| query_err("querying setting", e))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading setting row", e))?
        {
            let val: String = row
                .get(0)
                .map_err(|e| query_err("decoding setting value", e))?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// Stores or updates visual branding theme metadata for `server`.
    pub fn store_theme(&self, server: &str, theme: &CachedTheme) -> Result<(), StorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.database
            .connection()
            .execute(
                "INSERT INTO theme_cache (server, name, slogan, color, color_text, logo_path, background_url, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(server) DO UPDATE SET
                     name = excluded.name,
                     slogan = excluded.slogan,
                     color = excluded.color,
                     color_text = excluded.color_text,
                     logo_path = excluded.logo_path,
                     background_url = excluded.background_url,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    server,
                    theme.name.as_deref(),
                    theme.slogan.as_deref(),
                    theme.color.as_deref(),
                    theme.color_text.as_deref(),
                    theme.logo_path.as_deref(),
                    theme.background_url.as_deref(),
                    now,
                ],
            )
            .map_err(|e| query_err("storing theme in config store", e))?;

        Ok(())
    }

    /// Loads visual branding theme metadata for `server`.
    pub fn load_theme(&self, server: &str) -> Result<Option<CachedTheme>, StorageError> {
        let mut stmt = self
            .database
            .connection()
            .prepare(
                "SELECT server, name, slogan, color, color_text, logo_path, background_url, updated_at
                 FROM theme_cache WHERE server = ?1",
            )
            .map_err(|e| query_err("preparing select theme", e))?;

        let mut rows = stmt
            .query(rusqlite::params![server])
            .map_err(|e| query_err("querying theme", e))?;

        if let Some(row) = rows.next().map_err(|e| query_err("reading theme row", e))? {
            let server: String = row
                .get(0)
                .map_err(|e| query_err("reading theme server", e))?;
            let name: Option<String> =
                row.get(1).map_err(|e| query_err("reading theme name", e))?;
            let slogan: Option<String> = row
                .get(2)
                .map_err(|e| query_err("reading theme slogan", e))?;
            let color: Option<String> = row
                .get(3)
                .map_err(|e| query_err("reading theme color", e))?;
            let color_text: Option<String> = row
                .get(4)
                .map_err(|e| query_err("reading theme color_text", e))?;
            let logo_path: Option<String> = row
                .get(5)
                .map_err(|e| query_err("reading theme logo_path", e))?;
            let background_url: Option<String> = row
                .get(6)
                .map_err(|e| query_err("reading theme background_url", e))?;
            let updated_at: i64 = row
                .get(7)
                .map_err(|e| query_err("reading theme updated_at", e))?;

            Ok(Some(CachedTheme {
                server,
                name,
                slogan,
                color,
                color_text,
                logo_path,
                background_url,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Stores or updates a sync mount point configuration.
    pub fn store_sync_mount(&self, mount: &StoredSyncMount) -> Result<(), StorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.database
            .connection()
            .execute(
                "INSERT INTO sync_mount (id, account_id, remote_path, local_target, mode, display_name, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     account_id = excluded.account_id,
                     remote_path = excluded.remote_path,
                     local_target = excluded.local_target,
                     mode = excluded.mode,
                     display_name = excluded.display_name,
                     enabled = excluded.enabled,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    mount.id,
                    mount.account_id,
                    mount.remote_path,
                    mount.local_target,
                    mount.mode,
                    mount.display_name.as_deref(),
                    if mount.enabled { 1 } else { 0 },
                    mount.created_at,
                    now,
                ],
            )
            .map_err(|e| query_err("storing sync mount", e))?;

        Ok(())
    }

    /// Removes a sync mount by its unique identifier.
    pub fn remove_sync_mount(&self, id: &str) -> Result<(), StorageError> {
        self.database
            .connection()
            .execute(
                "DELETE FROM sync_mount WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| query_err("deleting sync mount", e))?;

        Ok(())
    }

    /// Lists all stored sync mounts for this account/SDK instance.
    pub fn list_sync_mounts(&self) -> Result<Vec<StoredSyncMount>, StorageError> {
        let mut stmt = self
            .database
            .connection()
            .prepare(
                "SELECT id, account_id, remote_path, local_target, mode, display_name, enabled, created_at, updated_at
                 FROM sync_mount ORDER BY created_at ASC",
            )
            .map_err(|e| query_err("preparing select sync_mounts", e))?;

        let mut rows = stmt
            .query([])
            .map_err(|e| query_err("querying sync_mounts", e))?;

        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading sync_mount row", e))?
        {
            let id: String = row.get(0).map_err(|e| query_err("reading id", e))?;
            let account_id: String = row.get(1).map_err(|e| query_err("reading account_id", e))?;
            let remote_path: String = row
                .get(2)
                .map_err(|e| query_err("reading remote_path", e))?;
            let local_target: String = row
                .get(3)
                .map_err(|e| query_err("reading local_target", e))?;
            let mode: String = row.get(4).map_err(|e| query_err("reading mode", e))?;
            let display_name: Option<String> = row
                .get(5)
                .map_err(|e| query_err("reading display_name", e))?;
            let enabled_int: i64 = row.get(6).map_err(|e| query_err("reading enabled", e))?;
            let created_at: i64 = row.get(7).map_err(|e| query_err("reading created_at", e))?;
            let updated_at: i64 = row.get(8).map_err(|e| query_err("reading updated_at", e))?;

            result.push(StoredSyncMount {
                id,
                account_id,
                remote_path,
                local_target,
                mode,
                display_name,
                enabled: enabled_int != 0,
                created_at,
                updated_at,
            });
        }

        Ok(result)
    }

    /// Stores directory sorting preferences in the configuration database.
    pub fn set_directory_sort(
        &self,
        dir_path: &str,
        sort_field: &str,
        sort_order: &str,
    ) -> Result<(), StorageError> {
        let clean = if dir_path.is_empty() || dir_path == "/" {
            "root".to_string()
        } else {
            dir_path.trim_matches('/').to_string()
        };
        let key = format!("dir_sort:{clean}");
        let val = format!("{sort_field}:{sort_order}");
        self.set_setting(&key, &val)
    }

    /// Loads directory sorting preferences from the configuration database.
    pub fn get_directory_sort(
        &self,
        dir_path: &str,
    ) -> Result<Option<(String, String)>, StorageError> {
        let clean = if dir_path.is_empty() || dir_path == "/" {
            "root".to_string()
        } else {
            dir_path.trim_matches('/').to_string()
        };
        let key = format!("dir_sort:{clean}");
        if let Some(val) = self.get_setting(&key)?
            && let Some((field, order)) = val.split_once(':')
        {
            return Ok(Some((field.to_string(), order.to_string())));
        }
        Ok(None)
    }
}

impl fmt::Debug for ConfigStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigStore")
            .field("path", &self.database.path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_sort_persistence() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("test_config_sort_{nanos}.db"));
        let key = StoreKey::generate().unwrap();
        let store = ConfigStore::open(&db_path, &key).unwrap();

        assert_eq!(store.get_directory_sort("/Photos").unwrap(), None);

        store.set_directory_sort("/Photos", "size", "desc").unwrap();
        assert_eq!(
            store.get_directory_sort("/Photos").unwrap(),
            Some(("size".to_string(), "desc".to_string()))
        );

        // Root directory
        store.set_directory_sort("/", "name", "asc").unwrap();
        assert_eq!(
            store.get_directory_sort("/").unwrap(),
            Some(("name".to_string(), "asc".to_string()))
        );
        assert_eq!(
            store.get_directory_sort("").unwrap(),
            Some(("name".to_string(), "asc".to_string()))
        );

        let _ = std::fs::remove_file(&db_path);
    }
}
