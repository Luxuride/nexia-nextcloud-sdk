//! Synchronization journal tracking state between sync cycles.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::SyncError;

/// An individual file or directory record stored in the sync journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Relative path within the synchronization root (e.g. `"/Documents/Notes.txt"`).
    pub relative_path: String,
    /// ETag recorded from Nextcloud server during last sync.
    pub remote_etag: Option<String>,
    /// File size in bytes recorded on local disk during last sync.
    pub local_size: u64,
    /// Local filesystem modification timestamp recorded during last sync.
    pub local_mtime: SystemTime,
    /// Whether this journal entry represents a directory.
    pub is_directory: bool,
}

impl JournalEntry {
    /// Creates a new journal record.
    pub fn new(
        relative_path: impl Into<String>,
        local_size: u64,
        local_mtime: SystemTime,
        is_directory: bool,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            remote_etag: None,
            local_size,
            local_mtime,
            is_directory,
        }
    }

    /// Sets the remote ETag on this entry.
    #[must_use]
    pub fn with_remote_etag(mut self, etag: impl Into<String>) -> Self {
        self.remote_etag = Some(etag.into());
        self
    }
}

/// Journal tracking synchronized file states to identify local and remote modifications.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncJournal {
    entries: HashMap<String, JournalEntry>,
}

impl SyncJournal {
    /// Creates a new empty synchronization journal.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Loads a journal from a reader.
    pub fn load_from_reader<R: std::io::Read>(reader: R) -> Result<Self, SyncError> {
        serde_json::from_reader(reader).map_err(|err| SyncError::Serialization {
            context: "reading sync journal stream".to_string(),
            source: err,
        })
    }

    /// Serializes the journal to a writer.
    pub fn save_to_writer<W: std::io::Write>(&self, writer: W) -> Result<(), SyncError> {
        serde_json::to_writer_pretty(writer, self).map_err(|err| SyncError::Serialization {
            context: "writing sync journal stream".to_string(),
            source: err,
        })
    }

    /// Loads a journal from a file path, returning an empty journal if the file does not exist.
    pub fn load_from_file(path: &Path) -> Result<Self, SyncError> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path).map_err(|err| SyncError::LocalIo {
            path: path.to_path_buf(),
            context: "reading sync journal file".to_string(),
            source: err,
        })?;

        serde_json::from_str(&content).map_err(|err| SyncError::Serialization {
            context: "parsing sync journal JSON".to_string(),
            source: err,
        })
    }

    /// Saves the journal atomically to a file path.
    pub fn save_to_file(&self, path: &Path) -> Result<(), SyncError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| SyncError::LocalIo {
                path: parent.to_path_buf(),
                context: "creating sync journal parent directory".to_string(),
                source: err,
            })?;
        }

        let json = serde_json::to_string_pretty(self).map_err(|err| SyncError::Serialization {
            context: "serializing sync journal JSON".to_string(),
            source: err,
        })?;

        fs::write(path, json).map_err(|err| SyncError::LocalIo {
            path: path.to_path_buf(),
            context: "writing sync journal file".to_string(),
            source: err,
        })
    }

    /// Looks up a journal entry by its relative path.
    pub fn get(&self, relative_path: &str) -> Option<&JournalEntry> {
        let clean = format!("/{}", relative_path.trim_matches('/'));
        self.entries.get(&clean)
    }

    /// Inserts or updates a journal entry.
    pub fn record(&mut self, mut entry: JournalEntry) {
        let clean = format!("/{}", entry.relative_path.trim_matches('/'));
        entry.relative_path = clean.clone();
        self.entries.insert(clean, entry);
    }

    /// Removes a journal entry by its relative path.
    pub fn remove(&mut self, relative_path: &str) -> Option<JournalEntry> {
        let clean = format!("/{}", relative_path.trim_matches('/'));
        self.entries.remove(&clean)
    }

    /// Returns the total number of tracked entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the journal contains no tracked entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all entries from the journal.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_persistence_roundtrip() {
        let tmp = std::env::temp_dir().join("nc_journal_sdk_test.json");
        let mut journal = SyncJournal::new();
        journal.record(JournalEntry {
            relative_path: "Docs/report.pdf".to_string(),
            remote_etag: Some("\"xyz\"".to_string()),
            local_size: 1024,
            local_mtime: SystemTime::now(),
            is_directory: false,
        });

        journal.save_to_file(&tmp).expect("saved successfully");
        let loaded = SyncJournal::load_from_file(&tmp).expect("loaded successfully");
        assert_eq!(loaded.len(), 1);
        let entry = loaded.get("/Docs/report.pdf").expect("found record");
        assert_eq!(entry.local_size, 1024);
        assert_eq!(entry.remote_etag.as_deref(), Some("\"xyz\""));

        let _ = fs::remove_file(tmp);
    }
}
