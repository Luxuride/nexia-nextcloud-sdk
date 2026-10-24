//! Metadata describing a virtual file or directory node.

use serde::{Deserialize, Serialize};

use crate::path::{file_name, normalize_path};

/// Rich metadata for a virtual file system node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Nextcloud numeric file identifier (`oc:fileid`), if reported by the server.
    pub file_id: Option<u64>,
    /// Display name of the entry (e.g. `"Notes.md"`).
    pub name: String,
    /// Canonical path within the virtual mount (e.g. `"/Documents/Notes.md"`).
    pub path: String,
    /// Whether this node represents a directory collection.
    pub is_directory: bool,
    /// Size of the resource in bytes as reported by metadata.
    pub size: u64,
    /// Entity tag (`ETag`) for cache validation and change detection.
    pub etag: Option<String>,
    /// Last modification timestamp as formatted string.
    pub modified_at: Option<String>,
    /// MIME type of the resource content (e.g. `"image/jpeg"`).
    pub mime_type: Option<String>,
    /// Whether the server reports thumbnail or preview generation support.
    pub has_preview: bool,
    /// Permission flags string reported by Nextcloud (e.g. `"RDNVW"`).
    pub permissions: Option<String>,
}

impl NodeMetadata {
    /// Constructs metadata for a file node with canonical path and initial defaults.
    pub fn file(path: impl Into<String>) -> Self {
        let raw_path = path.into();
        let canonical_path = normalize_path(&raw_path).unwrap_or(raw_path);
        let name = file_name(&canonical_path);
        Self {
            file_id: None,
            name,
            path: canonical_path,
            is_directory: false,
            size: 0,
            etag: None,
            modified_at: None,
            mime_type: None,
            has_preview: false,
            permissions: None,
        }
    }

    /// Constructs metadata for a directory node with canonical path and initial defaults.
    pub fn directory(path: impl Into<String>) -> Self {
        let raw_path = path.into();
        let canonical_path = normalize_path(&raw_path).unwrap_or(raw_path);
        let name = file_name(&canonical_path);
        Self {
            file_id: None,
            name,
            path: canonical_path,
            is_directory: true,
            size: 0,
            etag: None,
            modified_at: None,
            mime_type: Some("httpd/unix-directory".to_string()),
            has_preview: false,
            permissions: None,
        }
    }

    /// Sets the Nextcloud numeric file ID.
    #[must_use]
    pub const fn with_file_id(mut self, file_id: u64) -> Self {
        self.file_id = Some(file_id);
        self
    }

    /// Overrides the node display name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Sets the file size in bytes.
    #[must_use]
    pub const fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// Sets the ETag value.
    #[must_use]
    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }

    /// Sets the last modified date string.
    #[must_use]
    pub fn with_modified_at(mut self, modified_at: impl Into<String>) -> Self {
        self.modified_at = Some(modified_at.into());
        self
    }

    /// Sets the MIME type string.
    #[must_use]
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Sets whether preview generation is supported.
    #[must_use]
    pub const fn with_has_preview(mut self, has_preview: bool) -> Self {
        self.has_preview = has_preview;
        self
    }

    /// Sets permission string flags.
    #[must_use]
    pub fn with_permissions(mut self, permissions: impl Into<String>) -> Self {
        self.permissions = Some(permissions.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::NodeMetadata;

    #[test]
    fn file_metadata_builder() {
        let meta = NodeMetadata::file("/Documents/Notes.md")
            .with_file_id(42)
            .with_size(1024)
            .with_etag("etag123")
            .with_modified_at("2026-10-24T12:00:00Z")
            .with_mime_type("text/markdown")
            .with_has_preview(true)
            .with_permissions("RDNVW");

        assert_eq!(meta.name, "Notes.md");
        assert_eq!(meta.path, "/Documents/Notes.md");
        assert!(!meta.is_directory);
        assert_eq!(meta.file_id, Some(42));
        assert_eq!(meta.size, 1024);
        assert_eq!(meta.etag.as_deref(), Some("etag123"));
        assert_eq!(meta.modified_at.as_deref(), Some("2026-10-24T12:00:00Z"));
        assert_eq!(meta.mime_type.as_deref(), Some("text/markdown"));
        assert!(meta.has_preview);
        assert_eq!(meta.permissions.as_deref(), Some("RDNVW"));
    }

    #[test]
    fn directory_metadata_builder() {
        let meta = NodeMetadata::directory("/Photos");
        assert_eq!(meta.name, "Photos");
        assert_eq!(meta.path, "/Photos");
        assert!(meta.is_directory);
        assert_eq!(meta.mime_type.as_deref(), Some("httpd/unix-directory"));
    }
}
