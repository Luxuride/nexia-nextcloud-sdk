//! Metadata representing a past revision of a Nextcloud file.

use std::fmt;

/// A past revision of a Nextcloud file.
///
/// Returned by [`VersionsClient::list`](crate::VersionsClient::list).
/// Contains the WebDAV path to the revision, the revision identifier (timestamp),
/// and standard WebDAV properties such as last modification time, size in bytes,
/// and ETag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileVersion {
    /// Server-relative WebDAV path to this revision.
    pub href: String,
    /// Identifier of this revision, typically a Unix timestamp.
    pub revision_id: String,
    /// Last modified timestamp as an HTTP-date string (e.g. `Fri, 01 Jan 2021 00:00:00 GMT`), if reported.
    pub last_modified: Option<String>,
    /// Size of the revision in bytes, if reported.
    pub content_length: Option<u64>,
    /// Entity tag of this revision, if reported.
    pub etag: Option<String>,
}

impl FileVersion {
    /// Creates a new `FileVersion`.
    pub fn new(
        href: impl Into<String>,
        revision_id: impl Into<String>,
        last_modified: Option<String>,
        content_length: Option<u64>,
        etag: Option<String>,
    ) -> Self {
        Self {
            href: href.into(),
            revision_id: revision_id.into(),
            last_modified,
            content_length,
            etag,
        }
    }

    /// Server-relative WebDAV path to this revision.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Revision identifier.
    pub fn revision_id(&self) -> &str {
        &self.revision_id
    }

    /// Modification timestamp as an HTTP-date string, if available.
    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    /// Byte length of the revision content, if available.
    pub fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    /// Entity tag of the revision, if available.
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }
}

impl fmt::Display for FileVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FileVersion(revision_id={}, href={})",
            self.revision_id, self.href
        )
    }
}

#[cfg(test)]
mod tests {
    use super::FileVersion;

    #[test]
    fn file_version_accessors_and_display() {
        let version = FileVersion::new(
            "/remote.php/dav/versions/alice/versions/123/1609459200",
            "1609459200",
            Some("Fri, 01 Jan 2021 00:00:00 GMT".to_string()),
            Some(1024),
            Some("\"abcdef\"".to_string()),
        );

        assert_eq!(
            version.href(),
            "/remote.php/dav/versions/alice/versions/123/1609459200"
        );
        assert_eq!(version.revision_id(), "1609459200");
        assert_eq!(
            version.last_modified(),
            Some("Fri, 01 Jan 2021 00:00:00 GMT")
        );
        assert_eq!(version.content_length(), Some(1024));
        assert_eq!(version.etag(), Some("\"abcdef\""));
        assert_eq!(
            version.to_string(),
            "FileVersion(revision_id=1609459200, href=/remote.php/dav/versions/alice/versions/123/1609459200)"
        );
    }
}
