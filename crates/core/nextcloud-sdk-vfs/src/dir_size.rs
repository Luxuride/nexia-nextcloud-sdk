//! Directory size and nested item count aggregation model.

use serde::{Deserialize, Serialize};

use crate::category::format_bytes;

/// Aggregated size and item counts for a directory in the virtual tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySizeSummary {
    /// Total aggregate size in bytes of all descendant files.
    pub total_size: u64,
    /// Total number of nested descendant files.
    pub file_count: usize,
    /// Total number of nested descendant subdirectories.
    pub directory_count: usize,
}

impl DirectorySizeSummary {
    /// Constructs a new empty directory size summary.
    pub const fn empty() -> Self {
        Self {
            total_size: 0,
            file_count: 0,
            directory_count: 0,
        }
    }

    /// Creates a summary with explicit total size, file count, and directory count.
    pub const fn new(total_size: u64, file_count: usize, directory_count: usize) -> Self {
        Self {
            total_size,
            file_count,
            directory_count,
        }
    }

    /// Total number of nested items (files plus subdirectories).
    pub const fn total_items(&self) -> usize {
        self.file_count + self.directory_count
    }

    /// Whether this directory contains no files or subdirectories.
    pub const fn is_empty(&self) -> bool {
        self.file_count == 0 && self.directory_count == 0
    }

    /// Formats the byte size using human-readable units (`B`, `KB`, `MB`, `GB`).
    pub fn formatted_size(&self) -> String {
        format_bytes(self.total_size)
    }

    /// Formats a concise item count summary (e.g., `"5 files, 2 folders"`, `"1 file"`, `"Empty folder"`).
    pub fn items_description(&self) -> String {
        if self.is_empty() {
            return "Empty folder".to_string();
        }

        let files_str = match self.file_count {
            0 => None,
            1 => Some("1 file".to_string()),
            n => Some(format!("{n} files")),
        };

        let dirs_str = match self.directory_count {
            0 => None,
            1 => Some("1 folder".to_string()),
            n => Some(format!("{n} folders")),
        };

        match (files_str, dirs_str) {
            (Some(f), Some(d)) => format!("{f}, {d}"),
            (Some(f), None) => f,
            (None, Some(d)) => d,
            (None, None) => "Empty folder".to_string(),
        }
    }

    /// Formats a full detailed summary string (e.g. `"14.2 MB (28 files, 3 folders)"` or `"Empty folder"`).
    pub fn detailed_summary(&self) -> String {
        if self.is_empty() {
            "Empty folder".to_string()
        } else {
            format!("{} ({})", self.formatted_size(), self.items_description())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_directory_summary() {
        let summary = DirectorySizeSummary::empty();
        assert!(summary.is_empty());
        assert_eq!(summary.total_items(), 0);
        assert_eq!(summary.items_description(), "Empty folder");
        assert_eq!(summary.detailed_summary(), "Empty folder");
    }

    #[test]
    fn single_file_summary() {
        let summary = DirectorySizeSummary::new(1024, 1, 0);
        assert!(!summary.is_empty());
        assert_eq!(summary.total_items(), 1);
        assert_eq!(summary.items_description(), "1 file");
        assert_eq!(summary.detailed_summary(), "1.0 KB (1 file)");
    }

    #[test]
    fn single_folder_summary() {
        let summary = DirectorySizeSummary::new(0, 0, 1);
        assert!(!summary.is_empty());
        assert_eq!(summary.total_items(), 1);
        assert_eq!(summary.items_description(), "1 folder");
        assert_eq!(summary.detailed_summary(), "0 B (1 folder)");
    }

    #[test]
    fn mixed_contents_summary() {
        let summary = DirectorySizeSummary::new(15 * 1024 * 1024, 12, 3);
        assert_eq!(summary.total_items(), 15);
        assert_eq!(summary.items_description(), "12 files, 3 folders");
        assert_eq!(summary.detailed_summary(), "15.0 MB (12 files, 3 folders)");
    }
}
