use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sdk_core::ServerUrl;
use sdk_storage::{EncryptedDatabase, StorageError, StoreKey};

use super::cache_entry::CachedDavEntry;

fn query_err(operation: &'static str, source: rusqlite::Error) -> StorageError {
    StorageError::Query {
        operation,
        source: Box::new(source),
    }
}

/// Encrypted storage adapter for cached server capabilities and WebDAV metadata.
pub struct CacheStore {
    database: EncryptedDatabase,
}

impl CacheStore {
    /// Opens the encrypted cache database at `path` using `key`.
    pub fn open(path: impl AsRef<Path>, key: &StoreKey) -> Result<Self, StorageError> {
        let database = EncryptedDatabase::open_cache_store(path, key)?;
        Ok(Self { database })
    }

    /// Borrows the underlying encrypted database.
    pub const fn database(&self) -> &EncryptedDatabase {
        &self.database
    }

    /// Stores server capabilities for `server` and `user_id`.
    pub fn store_capabilities(
        &self,
        server: &ServerUrl,
        user_id: &str,
        body: &[u8],
    ) -> Result<(), StorageError> {
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.database
            .connection()
            .execute(
                "INSERT INTO capabilities (server, user_id, body, fetched_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(server) DO UPDATE SET
                     user_id = excluded.user_id,
                     body = excluded.body,
                     fetched_at = excluded.fetched_at",
                rusqlite::params![server.as_str(), user_id, body, fetched_at],
            )
            .map_err(|e| query_err("storing capabilities in cache", e))?;

        Ok(())
    }

    /// Loads cached capabilities for `server`, returning `(user_id, body)` if present.
    pub fn load_capabilities(
        &self,
        server: &ServerUrl,
    ) -> Result<Option<(String, Vec<u8>)>, StorageError> {
        let conn = self.database.connection();
        let mut stmt = conn
            .prepare("SELECT user_id, body FROM capabilities WHERE server = ?1 LIMIT 1")
            .map_err(|e| query_err("preparing load capabilities query", e))?;

        let mut rows = stmt
            .query(rusqlite::params![server.as_str()])
            .map_err(|e| query_err("querying capabilities", e))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading capabilities row", e))?
        {
            let user_id: String = row
                .get(0)
                .map_err(|e| query_err("reading user_id column", e))?;
            let body: Vec<u8> = row
                .get(1)
                .map_err(|e| query_err("reading body column", e))?;
            Ok(Some((user_id, body)))
        } else {
            Ok(None)
        }
    }

    /// Deletes cached capabilities for `server`.
    pub fn delete_capabilities(&self, server: &ServerUrl) -> Result<(), StorageError> {
        self.database
            .connection()
            .execute(
                "DELETE FROM capabilities WHERE server = ?1",
                rusqlite::params![server.as_str()],
            )
            .map_err(|e| query_err("deleting capabilities from cache", e))?;

        Ok(())
    }

    /// Stores or updates a cached WebDAV resource entry.
    pub fn store_dav_entry(
        &self,
        server: &ServerUrl,
        user_id: &str,
        entry: &CachedDavEntry,
    ) -> Result<(), StorageError> {
        self.database
            .connection()
            .execute(
                "INSERT INTO dav_entry (server, user_id, path, etag, content_type, size, modified_at, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(server, user_id, path) DO UPDATE SET
                     etag = excluded.etag,
                     content_type = excluded.content_type,
                     size = excluded.size,
                     modified_at = excluded.modified_at,
                     fetched_at = excluded.fetched_at",
                rusqlite::params![
                    server.as_str(),
                    user_id,
                    entry.path,
                    entry.etag,
                    entry.content_type,
                    entry.size,
                    entry.modified_at,
                    entry.fetched_at,
                ],
            )
            .map_err(|e| query_err("storing dav entry in cache", e))?;

        Ok(())
    }

    /// Loads a specific cached WebDAV entry by path.
    pub fn load_dav_entry(
        &self,
        server: &ServerUrl,
        user_id: &str,
        path: &str,
    ) -> Result<Option<CachedDavEntry>, StorageError> {
        let conn = self.database.connection();
        let mut stmt = conn
            .prepare(
                "SELECT path, etag, content_type, size, modified_at, fetched_at
                 FROM dav_entry
                 WHERE server = ?1 AND user_id = ?2 AND path = ?3
                 LIMIT 1",
            )
            .map_err(|e| query_err("preparing load dav entry query", e))?;

        let mut rows = stmt
            .query(rusqlite::params![server.as_str(), user_id, path])
            .map_err(|e| query_err("querying dav entry", e))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| query_err("reading dav entry row", e))?
        {
            Ok(Some(CachedDavEntry {
                path: row
                    .get(0)
                    .map_err(|e| query_err("reading path column", e))?,
                etag: row
                    .get(1)
                    .map_err(|e| query_err("reading etag column", e))?,
                content_type: row
                    .get(2)
                    .map_err(|e| query_err("reading content_type column", e))?,
                size: row
                    .get(3)
                    .map_err(|e| query_err("reading size column", e))?,
                modified_at: row
                    .get(4)
                    .map_err(|e| query_err("reading modified_at column", e))?,
                fetched_at: row
                    .get(5)
                    .map_err(|e| query_err("reading fetched_at column", e))?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Lists all cached WebDAV entries for `server` and `user_id`.
    pub fn list_dav_entries(
        &self,
        server: &ServerUrl,
        user_id: &str,
    ) -> Result<Vec<CachedDavEntry>, StorageError> {
        let conn = self.database.connection();
        let mut stmt = conn
            .prepare(
                "SELECT path, etag, content_type, size, modified_at, fetched_at
                 FROM dav_entry
                 WHERE server = ?1 AND user_id = ?2
                 ORDER BY path ASC",
            )
            .map_err(|e| query_err("preparing list dav entries query", e))?;

        let rows = stmt
            .query_map(rusqlite::params![server.as_str(), user_id], |row| {
                Ok(CachedDavEntry {
                    path: row.get(0)?,
                    etag: row.get(1)?,
                    content_type: row.get(2)?,
                    size: row.get(3)?,
                    modified_at: row.get(4)?,
                    fetched_at: row.get(5)?,
                })
            })
            .map_err(|e| query_err("querying dav entries list", e))?;

        let mut entries = Vec::new();
        for entry in rows {
            entries.push(entry.map_err(|e| query_err("reading entry in dav list", e))?);
        }

        Ok(entries)
    }

    /// Deletes a specific cached WebDAV entry.
    pub fn delete_dav_entry(
        &self,
        server: &ServerUrl,
        user_id: &str,
        path: &str,
    ) -> Result<(), StorageError> {
        self.database
            .connection()
            .execute(
                "DELETE FROM dav_entry WHERE server = ?1 AND user_id = ?2 AND path = ?3",
                rusqlite::params![server.as_str(), user_id, path],
            )
            .map_err(|e| query_err("deleting dav entry from cache", e))?;

        Ok(())
    }

    /// Wipes all cached data across all tables.
    pub fn clear(&self) -> Result<(), StorageError> {
        let conn = self.database.connection();
        conn.execute("DELETE FROM capabilities", [])
            .map_err(|e| query_err("clearing capabilities cache", e))?;
        conn.execute("DELETE FROM dav_entry", [])
            .map_err(|e| query_err("clearing dav cache", e))?;
        Ok(())
    }
}

impl fmt::Debug for CacheStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CacheStore")
            .field("path", &self.database.path())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cache-store-{name}-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn round_trip_capabilities() {
        let path = temp_path("caps");
        let key = StoreKey::generate().unwrap();
        let store = CacheStore::open(&path, &key).unwrap();
        let server = ServerUrl::parse("https://cloud.example.org").unwrap();

        assert!(store.load_capabilities(&server).unwrap().is_none());

        store
            .store_capabilities(&server, "alice", b"{\"version\": \"28.0\"}")
            .unwrap();

        let (uid, body) = store.load_capabilities(&server).unwrap().expect("found");
        assert_eq!(uid, "alice");
        assert_eq!(body, b"{\"version\": \"28.0\"}");

        // Updating
        store
            .store_capabilities(&server, "alice_new", b"{\"version\": \"29.0\"}")
            .unwrap();
        let (uid2, body2) = store.load_capabilities(&server).unwrap().expect("found");
        assert_eq!(uid2, "alice_new");
        assert_eq!(body2, b"{\"version\": \"29.0\"}");

        store.delete_capabilities(&server).unwrap();
        assert!(store.load_capabilities(&server).unwrap().is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn round_trip_dav_entries() {
        let path = temp_path("dav");
        let key = StoreKey::generate().unwrap();
        let store = CacheStore::open(&path, &key).unwrap();
        let server = ServerUrl::parse("https://cloud.example.org").unwrap();

        let entry = CachedDavEntry::new(
            "Documents/notes.txt",
            Some("etag1".to_string()),
            Some("text/plain".to_string()),
            Some(1024),
            Some("2026-10-24T12:00:00Z".to_string()),
            1700000000,
        );

        store.store_dav_entry(&server, "alice", &entry).unwrap();

        let loaded = store
            .load_dav_entry(&server, "alice", "Documents/notes.txt")
            .unwrap()
            .expect("found");
        assert_eq!(loaded, entry);

        let list = store.list_dav_entries(&server, "alice").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], entry);

        // Clear all
        store.clear().unwrap();
        assert!(
            store
                .load_dav_entry(&server, "alice", "Documents/notes.txt")
                .unwrap()
                .is_none()
        );
        assert!(store.list_dav_entries(&server, "alice").unwrap().is_empty());

        std::fs::remove_file(&path).ok();
    }
}
