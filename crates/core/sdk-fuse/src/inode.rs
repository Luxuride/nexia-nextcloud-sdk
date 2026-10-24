//! Inode allocator and path mapping table for FUSE operations.

use std::collections::HashMap;

/// The canonical FUSE root inode number (per libfuse standard).
pub const FUSE_ROOT_INODE: u64 = 1;

/// Bidirectional map between 64-bit FUSE inode identifiers and VFS canonical paths.
#[derive(Debug, Clone)]
pub struct InodeTable {
    next_inode: u64,
    inode_to_path: HashMap<u64, String>,
    path_to_inode: HashMap<String, u64>,
}

impl Default for InodeTable {
    fn default() -> Self {
        Self::new()
    }
}

impl InodeTable {
    /// Constructs a new [`InodeTable`] with the root path (`/`) assigned to inode 1.
    pub fn new() -> Self {
        let mut inode_to_path = HashMap::new();
        let mut path_to_inode = HashMap::new();

        inode_to_path.insert(FUSE_ROOT_INODE, "/".to_string());
        path_to_inode.insert("/".to_string(), FUSE_ROOT_INODE);

        Self {
            next_inode: FUSE_ROOT_INODE + 1,
            inode_to_path,
            path_to_inode,
        }
    }

    /// Retrieves or allocates an inode for a given canonical path.
    pub fn get_or_insert(&mut self, path: &str) -> u64 {
        if let Some(&inode) = self.path_to_inode.get(path) {
            return inode;
        }

        let inode = self.next_inode;
        self.next_inode += 1;

        self.inode_to_path.insert(inode, path.to_string());
        self.path_to_inode.insert(path.to_string(), inode);
        inode
    }

    /// Looks up the canonical path associated with an inode number.
    pub fn path_by_inode(&self, inode: u64) -> Option<&str> {
        self.inode_to_path.get(&inode).map(String::as_str)
    }

    /// Looks up the inode number associated with a canonical path, if mapped.
    pub fn inode_by_path(&self, path: &str) -> Option<u64> {
        self.path_to_inode.get(path).copied()
    }

    /// Removes an inode entry when a path is deleted.
    pub fn remove_by_path(&mut self, path: &str) -> Option<u64> {
        if let Some(inode) = self.path_to_inode.remove(path) {
            self.inode_to_path.remove(&inode);
            Some(inode)
        } else {
            None
        }
    }

    /// Total number of mapped paths.
    pub fn len(&self) -> usize {
        self.inode_to_path.len()
    }

    /// Returns `true` if no inodes are mapped.
    pub fn is_empty(&self) -> bool {
        self.inode_to_path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_inode_is_always_one() {
        let table = InodeTable::new();
        assert_eq!(table.path_by_inode(FUSE_ROOT_INODE), Some("/"));
        assert_eq!(table.inode_by_path("/"), Some(FUSE_ROOT_INODE));
    }

    #[test]
    fn allocates_monotonic_inodes() {
        let mut table = InodeTable::new();
        let ino1 = table.get_or_insert("/folder");
        let ino2 = table.get_or_insert("/folder/file.txt");

        assert_eq!(ino1, 2);
        assert_eq!(ino2, 3);
        assert_eq!(table.get_or_insert("/folder"), 2);
        assert_eq!(table.path_by_inode(ino2), Some("/folder/file.txt"));
    }

    #[test]
    fn removal_cleans_both_maps() {
        let mut table = InodeTable::new();
        let ino = table.get_or_insert("/tmp.txt");
        assert_eq!(table.remove_by_path("/tmp.txt"), Some(ino));
        assert_eq!(table.path_by_inode(ino), None);
        assert_eq!(table.inode_by_path("/tmp.txt"), None);
    }
}
