//! Directory entry structures for FUSE `readdir` operations.

use crate::attr::FileType;

/// An individual directory child record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// Inode of the entry.
    pub ino: u64,
    /// Offset index within the directory stream.
    pub offset: i64,
    /// Classification of the child node.
    pub kind: FileType,
    /// Display file or directory name.
    pub name: String,
}

impl DirEntry {
    /// Constructs a new [`DirEntry`].
    pub fn new(ino: u64, offset: i64, kind: FileType, name: impl Into<String>) -> Self {
        Self {
            ino,
            offset,
            kind,
            name: name.into(),
        }
    }
}
