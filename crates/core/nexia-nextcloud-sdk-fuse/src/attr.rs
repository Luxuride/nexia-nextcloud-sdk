//! File attributes and types for FUSE nodes.

use std::time::SystemTime;

/// Preferred I/O block size reported to the kernel.
///
/// This is what clients use to size their read requests. Reporting 512 made viewers issue
/// 512-byte reads, which forced one remote fetch per block; 128 KiB matches the hydration block
/// size and keeps large files paging in a handful of round trips.
pub const BLOCK_SIZE: u64 = 128 * 1024;

/// Node type classification for FUSE filesystem entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Regular file node.
    RegularFile,
    /// Directory node.
    Directory,
}

/// Standard filesystem metadata attributes reported by FUSE operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAttr {
    /// The unique inode identifier.
    pub ino: u64,
    /// Total size in bytes.
    pub size: u64,
    /// Number of 512-byte blocks allocated (the POSIX `st_blocks` unit is fixed).
    pub blocks: u64,
    /// Time of last access.
    pub atime: SystemTime,
    /// Time of last modification.
    pub mtime: SystemTime,
    /// Time of last status change.
    pub ctime: SystemTime,
    /// Time of creation.
    pub crtime: SystemTime,
    /// File type (directory or regular file).
    pub kind: FileType,
    /// POSIX file permissions mode.
    pub perm: u16,
    /// Number of hard links.
    pub nlink: u32,
    /// User ID of owner.
    pub uid: u32,
    /// Group ID of owner.
    pub gid: u32,
    /// Device identifier.
    pub rdev: u32,
    /// Flags.
    pub flags: u32,
}

impl FileAttr {
    /// Constructs a [`FileAttr`] with standard default permissions.
    pub fn new(ino: u64, kind: FileType, size: u64, mtime: SystemTime) -> Self {
        let (perm, nlink) = match kind {
            FileType::Directory => (0o755, 2),
            FileType::RegularFile => (0o644, 1),
        };
        let blocks = size.div_ceil(512);
        debug_assert!(BLOCK_SIZE.is_multiple_of(512));

        Self {
            ino,
            size,
            blocks,
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm,
            nlink,
            uid: 1000,
            gid: 1000,
            rdev: 0,
            flags: 0,
        }
    }

    /// Returns `true` if this attribute describes a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, FileType::Directory)
    }

    /// Returns `true` if this attribute describes a regular file.
    pub fn is_file(&self) -> bool {
        matches!(self.kind, FileType::RegularFile)
    }
}
