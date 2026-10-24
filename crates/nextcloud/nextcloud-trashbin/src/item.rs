//! Domain model representing a deleted item in Nextcloud's trashbin.

use std::fmt;

/// A deleted item in a user's Nextcloud trashbin.
///
/// Returned by [`TrashClient::list`](crate::TrashClient::list). Holds the
/// metadata reported by Nextcloud's WebDAV trashbin endpoint, including its
/// identifier, original filename, original location, deletion timestamp,
/// and WebDAV properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItem {
    /// Identifier of the trash item (the path segment in the trashbin URL).
    pub id: String,
    /// WebDAV resource href reported by the server.
    pub href: String,
    /// Original file or directory name before deletion (`trashbin-filename`).
    pub filename: String,
    /// Path where the item resided before deletion (`trashbin-original-location`).
    pub original_location: String,
    /// Unix timestamp (in seconds) when the item was moved to the trash (`trashbin-deletion-time`).
    pub deletion_time: Option<u64>,
    /// Last modified time as reported by the server (`d:getlastmodified`).
    pub last_modified: Option<String>,
    /// Whether the resource is a collection/folder (`<d:collection/>`).
    pub is_collection: bool,
}

impl TrashItem {
    /// Identifier of the trash item.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// WebDAV href of the item.
    pub fn href(&self) -> &str {
        &self.href
    }

    /// Original filename before deletion.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Original filename before deletion (alias for [`filename`](Self::filename)).
    pub fn original_filename(&self) -> &str {
        &self.filename
    }

    /// Original path where the item resided before deletion.
    pub fn original_location(&self) -> &str {
        &self.original_location
    }

    /// Unix timestamp when the item was deleted, if available.
    pub fn deletion_time(&self) -> Option<u64> {
        self.deletion_time
    }

    /// Last modified HTTP date string, if available.
    pub fn last_modified(&self) -> Option<&str> {
        self.last_modified.as_deref()
    }

    /// Whether the item is a collection (folder).
    pub fn is_collection(&self) -> bool {
        self.is_collection
    }
}

impl fmt::Display for TrashItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.is_collection {
            "collection"
        } else {
            "file"
        };
        write!(f, "{kind} {} (id: {})", self.filename, self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::TrashItem;

    #[test]
    fn item_accessors_and_display() {
        let item = TrashItem {
            id: "report.pdf.d1609459200".to_owned(),
            href: "/remote.php/dav/trashbin/alice/trash/report.pdf.d1609459200".to_owned(),
            filename: "report.pdf".to_owned(),
            original_location: "Documents/report.pdf".to_owned(),
            deletion_time: Some(1609459200),
            last_modified: Some("Fri, 01 Jan 2021 00:00:00 GMT".to_owned()),
            is_collection: false,
        };

        assert_eq!(item.id(), "report.pdf.d1609459200");
        assert_eq!(
            item.href(),
            "/remote.php/dav/trashbin/alice/trash/report.pdf.d1609459200"
        );
        assert_eq!(item.filename(), "report.pdf");
        assert_eq!(item.original_filename(), "report.pdf");
        assert_eq!(item.original_location(), "Documents/report.pdf");
        assert_eq!(item.deletion_time(), Some(1609459200));
        assert_eq!(item.last_modified(), Some("Fri, 01 Jan 2021 00:00:00 GMT"));
        assert!(!item.is_collection());

        let rendered = item.to_string();
        assert_eq!(rendered, "file report.pdf (id: report.pdf.d1609459200)");

        let dir = TrashItem {
            id: "Photos.d1609459300".to_owned(),
            href: "/remote.php/dav/trashbin/alice/trash/Photos.d1609459300/".to_owned(),
            filename: "Photos".to_owned(),
            original_location: "Photos".to_owned(),
            deletion_time: None,
            last_modified: None,
            is_collection: true,
        };
        assert!(dir.is_collection());
        assert_eq!(
            dir.to_string(),
            "collection Photos (id: Photos.d1609459300)"
        );
    }
}
