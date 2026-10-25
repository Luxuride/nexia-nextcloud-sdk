//! File and directory sorting criteria and comparator utilities.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::entry::VirtualEntrySummary;

/// Property to sort virtual directory entries by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSortField {
    /// Sort alphabetically by file or folder name.
    #[default]
    Name,
    /// Sort by file size in bytes (directories use total recursive size if available).
    Size,
    /// Sort chronologically by last modified timestamp.
    Modified,
    /// Sort by file type or extension.
    Type,
}

/// Direction of sorting order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileSortOrder {
    /// Ascending order (A to Z, smallest to largest, oldest to newest).
    #[default]
    Ascending,
    /// Descending order (Z to A, largest to smallest, newest to oldest).
    Descending,
}

impl FileSortOrder {
    /// Returns the inverted sort order.
    #[must_use]
    pub const fn invert(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Complete file sorting criteria specifying field, direction, and directory priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileSortCriteria {
    /// The primary attribute field to sort by.
    pub field: FileSortField,
    /// The sort direction (ascending or descending).
    pub order: FileSortOrder,
    /// Whether directories are pinned to always appear before regular files.
    pub folders_first: bool,
}

impl Default for FileSortCriteria {
    fn default() -> Self {
        Self {
            field: FileSortField::Name,
            order: FileSortOrder::Ascending,
            folders_first: true,
        }
    }
}

impl FileSortCriteria {
    /// Creates a new sorting criteria with specified field, order, and folder priority.
    #[must_use]
    pub const fn new(field: FileSortField, order: FileSortOrder, folders_first: bool) -> Self {
        Self {
            field,
            order,
            folders_first,
        }
    }

    /// Sorts a slice of [`VirtualEntrySummary`] in place according to these criteria.
    pub fn sort_entries(&self, entries: &mut [VirtualEntrySummary]) {
        entries.sort_by(|a, b| self.compare(a, b));
    }

    /// Compares two [`VirtualEntrySummary`] entries using these criteria.
    #[must_use]
    pub fn compare(&self, a: &VirtualEntrySummary, b: &VirtualEntrySummary) -> Ordering {
        if self.folders_first && a.is_directory != b.is_directory {
            return if a.is_directory {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let order = match self.field {
            FileSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            FileSortField::Size => a.size.cmp(&b.size),
            FileSortField::Modified => a.modified_at.cmp(&b.modified_at),
            FileSortField::Type => {
                let ext_a = std::path::Path::new(&a.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let ext_b = std::path::Path::new(&b.name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                ext_a
                    .cmp(&ext_b)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
        };

        match self.order {
            FileSortOrder::Ascending => order,
            FileSortOrder::Descending => order.reverse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::ResourceIconCategory;

    fn make_entry(
        name: &str,
        is_directory: bool,
        size: u64,
        modified: Option<&str>,
    ) -> VirtualEntrySummary {
        VirtualEntrySummary {
            path: format!("/{name}"),
            name: name.to_string(),
            is_directory,
            size,
            etag: None,
            modified_at: modified.map(ToString::to_string),
            permissions: None,
            mime_type: None,
            icon_category: if is_directory {
                ResourceIconCategory::Folder
            } else {
                ResourceIconCategory::Document
            },
            is_hydrated: false,
            cached_path: None,
            file_id: None,
            directory_summary: None,
        }
    }

    #[test]
    fn test_sort_by_name_ascending_folders_first() {
        let criteria = FileSortCriteria::default();
        let mut entries = vec![
            make_entry("zebra.txt", false, 100, None),
            make_entry("apple.txt", false, 50, None),
            make_entry("Documents", true, 0, None),
        ];

        criteria.sort_entries(&mut entries);

        assert_eq!(entries[0].name, "Documents");
        assert_eq!(entries[1].name, "apple.txt");
        assert_eq!(entries[2].name, "zebra.txt");
    }

    #[test]
    fn test_sort_by_size_descending() {
        let criteria = FileSortCriteria::new(FileSortField::Size, FileSortOrder::Descending, false);
        let mut entries = vec![
            make_entry("small.txt", false, 10, None),
            make_entry("large.txt", false, 1000, None),
            make_entry("medium.txt", false, 200, None),
        ];

        criteria.sort_entries(&mut entries);

        assert_eq!(entries[0].name, "large.txt");
        assert_eq!(entries[1].name, "medium.txt");
        assert_eq!(entries[2].name, "small.txt");
    }

    #[test]
    fn test_sort_by_type() {
        let criteria = FileSortCriteria::new(FileSortField::Type, FileSortOrder::Ascending, false);
        let mut entries = vec![
            make_entry("b.png", false, 10, None),
            make_entry("a.jpg", false, 10, None),
            make_entry("c.pdf", false, 10, None),
        ];

        criteria.sort_entries(&mut entries);

        assert_eq!(entries[0].name, "a.jpg");
        assert_eq!(entries[1].name, "c.pdf");
        assert_eq!(entries[2].name, "b.png");
    }

    #[test]
    fn test_invert_order() {
        assert_eq!(FileSortOrder::Ascending.invert(), FileSortOrder::Descending);
        assert_eq!(FileSortOrder::Descending.invert(), FileSortOrder::Ascending);
    }
}
