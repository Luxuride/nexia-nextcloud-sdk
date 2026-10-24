/// A cached WebDAV resource entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedDavEntry {
    /// Path of the resource in the user's WebDAV namespace.
    pub path: String,
    /// ETag identifier, if reported by the server.
    pub etag: Option<String>,
    /// MIME content type, if known.
    pub content_type: Option<String>,
    /// Size in bytes, if known.
    pub size: Option<i64>,
    /// Last modified timestamp as reported by the server.
    pub modified_at: Option<String>,
    /// Unix timestamp in seconds when this record was cached.
    pub fetched_at: i64,
}

impl CachedDavEntry {
    /// Constructs a new cached WebDAV entry.
    pub fn new(
        path: impl Into<String>,
        etag: Option<String>,
        content_type: Option<String>,
        size: Option<i64>,
        modified_at: Option<String>,
        fetched_at: i64,
    ) -> Self {
        Self {
            path: path.into(),
            etag,
            content_type,
            size,
            modified_at,
            fetched_at,
        }
    }
}
