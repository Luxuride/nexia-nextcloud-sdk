//! FUSE driver implementation over [`VirtualTree`].

use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use nextcloud_sdk_vfs::{HydrationState, VirtualNode, VirtualTree};

use crate::attr::{FileAttr, FileType};
use crate::entry::DirEntry;
use crate::hydrator::{AsyncHydrator, NoopHydrator};
use crate::inode::{FUSE_ROOT_INODE, InodeTable};

/// Standard POSIX error constant: No such file or directory.
pub const ENOENT: i32 = 2;
/// Standard POSIX error constant: Invalid argument.
pub const EINVAL: i32 = 22;
/// Standard POSIX error constant: Not a directory.
pub const ENOTDIR: i32 = 20;

/// Core filesystem driver bridging a [`VirtualTree`] and [`InodeTable`] to FUSE operations.
pub struct FuseDriver {
    tree: Arc<RwLock<VirtualTree>>,
    inodes: Arc<RwLock<InodeTable>>,
    hydrator: Arc<dyn AsyncHydrator>,
    runtime: Option<tokio::runtime::Handle>,
}

impl FuseDriver {
    /// Constructs a new [`FuseDriver`] over a [`VirtualTree`] with a default no-op hydrator.
    pub fn new(tree: Arc<RwLock<VirtualTree>>) -> Self {
        Self::with_hydrator(tree, Arc::new(NoopHydrator))
    }

    /// Constructs a new [`FuseDriver`] with an explicit payload hydrator.
    pub fn with_hydrator(tree: Arc<RwLock<VirtualTree>>, hydrator: Arc<dyn AsyncHydrator>) -> Self {
        let mut inodes = InodeTable::new();

        // Pre-populate inodes for existing nodes in the tree
        if let Ok(tree_guard) = tree.read() {
            for path in tree_guard.paths() {
                if path != "/" {
                    inodes.get_or_insert(path);
                }
            }
        }

        Self {
            tree,
            inodes: Arc::new(RwLock::new(inodes)),
            hydrator,
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    /// Configures the tokio runtime handle for executing async hydration during FUSE operations.
    pub fn with_runtime(mut self, runtime: tokio::runtime::Handle) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Returns the optional tokio runtime handle used for hydration.
    pub fn runtime_handle(&self) -> Option<tokio::runtime::Handle> {
        self.runtime.clone()
    }

    /// Looks up a child node by `name` inside the parent directory identified by `parent_ino`.
    pub fn lookup(&self, parent_ino: u64, name: &str) -> Result<FileAttr, i32> {
        tracing::trace!(parent_ino, name, "FUSE lookup");
        let parent_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(parent_ino).ok_or(ENOENT)?.to_string()
        };

        let child_path = if parent_path == "/" {
            format!("/{name}")
        } else {
            format!("{parent_path}/{name}")
        };

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&child_path).ok_or(ENOENT)?;

        let mut inodes = self.inodes.write().map_err(|_| EINVAL)?;
        let ino = inodes.get_or_insert(&child_path);

        Ok(Self::node_to_attr(ino, node))
    }

    /// Retrieves attributes for the node identified by `ino`.
    pub fn getattr(&self, ino: u64) -> Result<FileAttr, i32> {
        let path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&path).ok_or(ENOENT)?;

        Ok(Self::node_to_attr(ino, node))
    }

    /// Reads directory child entries for directory `ino` starting from `offset`.
    pub fn readdir(&self, ino: u64, offset: i64) -> Result<Vec<DirEntry>, i32> {
        let path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&path).ok_or(ENOENT)?;

        if !node.is_directory() {
            return Err(ENOTDIR);
        }

        let mut entries = Vec::new();

        // 1. Current entry (.)
        entries.push(DirEntry::new(ino, 1, FileType::Directory, "."));

        // 2. Parent entry (..)
        let parent_ino = if ino == FUSE_ROOT_INODE {
            FUSE_ROOT_INODE
        } else {
            let parent_path =
                nextcloud_sdk_vfs::parent_path(&path).unwrap_or_else(|| "/".to_string());
            let mut inodes = self.inodes.write().map_err(|_| EINVAL)?;
            inodes.get_or_insert(&parent_path)
        };
        entries.push(DirEntry::new(parent_ino, 2, FileType::Directory, ".."));

        // 3. Children
        self.hydrator.on_directory_read(&path);
        let children = tree.children(&path);

        let mut inodes_guard = self.inodes.write().map_err(|_| EINVAL)?;
        let mut preview_candidates = Vec::new();
        for (cur_offset, child) in (3..).zip(children) {
            let child_ino = inodes_guard.get_or_insert(child.path());
            let kind = if child.is_directory() {
                FileType::Directory
            } else {
                if child.metadata.has_preview || is_preview_name(child.name()) {
                    preview_candidates.push(child.path().to_string());
                }
                FileType::RegularFile
            };
            entries.push(DirEntry::new(child_ino, cur_offset, kind, child.name()));
        }
        drop(inodes_guard);

        if !preview_candidates.is_empty() {
            self.hydrator.on_readdir(&preview_candidates);
        }

        let start_idx = (offset as usize).min(entries.len());
        Ok(entries[start_idx..].to_vec())
    }

    /// Reads bytes from regular file `ino` at `offset` up to `size` bytes.
    ///
    /// If the file is an unhydrated placeholder, transparently triggers the hydrator.
    pub async fn read(&self, ino: u64, offset: i64, size: u32) -> Result<Vec<u8>, i32> {
        let path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };
        tracing::trace!(ino, path = %path, offset, size, "FUSE read request");

        // Check if placeholder
        let is_placeholder = {
            let tree = self.tree.read().map_err(|_| EINVAL)?;
            let node = tree.get(&path).ok_or(ENOENT)?;
            node.is_placeholder()
        };

        if is_placeholder {
            tracing::info!(ino, path = %path, "FUSE triggering on-demand file hydration");
            let bytes = self.hydrator.hydrate(&path).await.map_err(|_| ENOENT)?;
            let mut tree = self.tree.write().map_err(|_| EINVAL)?;
            tree.hydrate(&path, bytes).map_err(|_| EINVAL)?;
        }

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&path).ok_or(ENOENT)?;

        if let HydrationState::Hydrated(content) = node.hydration()
            && let Some(bytes) = content.as_bytes()
        {
            let off = (offset as usize).min(bytes.len());
            let end = (off + size as usize).min(bytes.len());
            return Ok(bytes[off..end].to_vec());
        }

        Ok(Vec::new())
    }

    /// Retrieves extended attributes such as thumbnails (`user.thumb.{width}x{height}`).
    pub fn getxattr(&self, ino: u64, name: &str) -> Result<Vec<u8>, i32> {
        let path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        if let Some(dim_str) = name.strip_prefix("user.thumb.")
            && let Some((w_str, h_str)) = dim_str.split_once('x')
            && let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>())
        {
            let tree = self.tree.read().map_err(|_| EINVAL)?;
            if let Some(bytes) = tree.get_thumbnail(&path, w, h) {
                return Ok(bytes.to_vec());
            }
        }

        Err(ENOENT)
    }

    /// Lists supported extended attribute names for `ino`.
    pub fn listxattr(&self, ino: u64) -> Result<Vec<String>, i32> {
        let path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&path).ok_or(ENOENT)?;

        let mut attrs = Vec::new();
        for ((w, h), _) in node.thumbnails().iter() {
            attrs.push(format!("user.thumb.{w}x{h}"));
        }

        Ok(attrs)
    }

    fn node_to_attr(ino: u64, node: &VirtualNode) -> FileAttr {
        let kind = if node.is_directory() {
            FileType::Directory
        } else {
            FileType::RegularFile
        };
        let mtime = parse_node_mtime(node.metadata.modified_at.as_deref());

        FileAttr::new(ino, kind, node.size(), mtime)
    }
}

fn is_preview_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
        || lower.ends_with(".mp4")
        || lower.ends_with(".mkv")
        || lower.ends_with(".webm")
        || lower.ends_with(".mov")
        || lower.ends_with(".avi")
        || lower.ends_with(".pdf")
}

fn parse_node_mtime(s: Option<&str>) -> SystemTime {
    let Some(raw) = s else {
        return SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    };

    let trimmed = raw.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    }
    if let Some(secs) = parse_http_date(trimmed) {
        return SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    }
    if let Some(secs) = parse_iso_date(trimmed) {
        return SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    }

    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)
}

fn parse_month(m: &str) -> Option<u64> {
    match m.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn civil_to_unix_secs(y: i64, m: u64, d: u64, h: u64, min: u64, sec: u64) -> Option<u64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || min > 59 || sec > 60 {
        return None;
    }
    let y = y - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe as i64 - 719468;
    if days < 0 {
        return None;
    }
    Some((days as u64) * 86400 + h * 3600 + min * 60 + sec)
}

fn parse_http_date(s: &str) -> Option<u64> {
    let after_comma = if let Some((_, rest)) = s.split_once(',') {
        rest.trim()
    } else {
        s.trim()
    };
    let parts: Vec<&str> = after_comma.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let day = parts[0].parse::<u64>().ok()?;
    let month = parse_month(parts[1])?;
    let year = parts[2].parse::<i64>().ok()?;
    let time_parts: Vec<&str> = parts[3].split(':').collect();
    if time_parts.len() < 3 {
        return None;
    }
    let hour = time_parts[0].parse::<u64>().ok()?;
    let min = time_parts[1].parse::<u64>().ok()?;
    let sec = time_parts[2].parse::<u64>().ok()?;

    civil_to_unix_secs(year, month, day, hour, min, sec)
}

fn parse_iso_date(s: &str) -> Option<u64> {
    let (date_part, time_part) = if let Some((d, t)) = s.split_once('T') {
        (d, t)
    } else if let Some((d, t)) = s.split_once(' ') {
        (d, t)
    } else {
        return None;
    };

    let d_parts: Vec<&str> = date_part.split('-').collect();
    if d_parts.len() != 3 {
        return None;
    }
    let year = d_parts[0].parse::<i64>().ok()?;
    let month = d_parts[1].parse::<u64>().ok()?;
    let day = d_parts[2].parse::<u64>().ok()?;

    let clean_time = time_part.trim_end_matches('Z');
    let clean_time = clean_time.split('+').next().unwrap_or(clean_time);
    let t_parts: Vec<&str> = clean_time.split(':').collect();
    if t_parts.len() < 2 {
        return None;
    }
    let hour = t_parts[0].parse::<u64>().ok()?;
    let min = t_parts[1].parse::<u64>().ok()?;
    let sec = if t_parts.len() >= 3 {
        t_parts[2]
            .split('.')
            .next()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0)
    } else {
        0
    };

    civil_to_unix_secs(year, month, day, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nextcloud_sdk_vfs::{NodeMetadata, VfsError};
    use std::pin::Pin;

    struct TestHydrator;

    impl AsyncHydrator for TestHydrator {
        fn hydrate(
            &self,
            _canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            Box::pin(async { Ok(b"hydrated payload content".to_vec()) })
        }
    }

    #[tokio::test]
    async fn root_getattr_and_readdir() {
        let mut tree = VirtualTree::new("root");
        let file_meta = NodeMetadata::file("/doc.txt").with_size(24);
        tree.insert(VirtualNode::new_file(file_meta));

        let driver = FuseDriver::new(Arc::new(RwLock::new(tree)));

        let root_attr = driver.getattr(FUSE_ROOT_INODE).unwrap();
        assert!(root_attr.is_dir());

        let entries = driver.readdir(FUSE_ROOT_INODE, 0).unwrap();
        assert_eq!(entries.len(), 3); // ., .., doc.txt
        assert_eq!(entries[2].name, "doc.txt");
    }

    #[tokio::test]
    async fn lookup_and_on_demand_read_hydration() {
        let mut tree = VirtualTree::new("root");
        let file_meta = NodeMetadata::file("/test.txt").with_size(24);
        tree.insert(VirtualNode::new_file(file_meta));

        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), Arc::new(TestHydrator));

        let attr = driver.lookup(FUSE_ROOT_INODE, "test.txt").unwrap();
        assert!(attr.is_file());
        assert_eq!(attr.size, 24);

        let bytes = driver.read(attr.ino, 0, 8).await.unwrap();
        assert_eq!(bytes, b"hydrated");
    }

    #[test]
    fn thumbnail_xattr_exposure() {
        let mut tree = VirtualTree::new("root");
        let file_meta = NodeMetadata::file("/photo.jpg");
        tree.insert(VirtualNode::new_file(file_meta));
        tree.set_thumbnail("/photo.jpg", 128, 128, vec![1, 2, 3, 4])
            .unwrap();

        let driver = FuseDriver::new(Arc::new(RwLock::new(tree)));
        let attr = driver.lookup(FUSE_ROOT_INODE, "photo.jpg").unwrap();

        let xattrs = driver.listxattr(attr.ino).unwrap();
        assert_eq!(xattrs, vec!["user.thumb.128x128"]);

        let bytes = driver.getxattr(attr.ino, "user.thumb.128x128").unwrap();
        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn node_mtime_parsing_and_stability() {
        let http_time = parse_node_mtime(Some("Wed, 21 Oct 2015 07:28:00 GMT"));
        let dur = http_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(dur.as_secs(), 1445412480);

        let iso_time = parse_node_mtime(Some("2026-10-24T12:00:00Z"));
        let dur2 = iso_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        assert!(dur2.as_secs() > 1700000000);

        let raw_time = parse_node_mtime(Some("1600000000"));
        let dur3 = raw_time.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(dur3.as_secs(), 1600000000);
    }
}
