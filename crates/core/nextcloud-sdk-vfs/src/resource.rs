//! WebDAV resource metadata summary for VFS population.

use serde::{Deserialize, Serialize};

/// Lightweight summary of a discovered remote WebDAV resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DavResourceSummary {
    /// Remote resource URI or href.
    pub href: String,
    /// Whether this resource is a collection/directory.
    pub is_collection: bool,
    /// Content length in bytes, if available.
    pub content_length: Option<u64>,
    /// Entity tag (ETag) identifier.
    pub etag: Option<String>,
    /// Content MIME type, if available.
    pub content_type: Option<String>,
    /// Nextcloud internal numeric file ID (`oc:fileid`), if available.
    pub file_id: Option<u64>,
    /// Whether the Nextcloud instance advertises a thumbnail preview for this resource.
    pub has_preview: Option<bool>,
}

impl DavResourceSummary {
    /// Creates a new resource summary.
    pub fn new(href: impl Into<String>, is_collection: bool) -> Self {
        Self {
            href: href.into(),
            is_collection,
            content_length: None,
            etag: None,
            content_type: None,
            file_id: None,
            has_preview: None,
        }
    }

    /// Sets the file ID.
    #[must_use]
    pub const fn with_file_id(mut self, file_id: u64) -> Self {
        self.file_id = Some(file_id);
        self
    }

    /// Sets the file size in bytes.
    #[must_use]
    pub const fn with_content_length(mut self, length: u64) -> Self {
        self.content_length = Some(length);
        self
    }

    /// Sets the resource ETag.
    #[must_use]
    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }

    /// Sets the MIME content type.
    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets whether the item has a server preview.
    #[must_use]
    pub const fn with_has_preview(mut self, has_preview: bool) -> Self {
        self.has_preview = Some(has_preview);
        self
    }
}
