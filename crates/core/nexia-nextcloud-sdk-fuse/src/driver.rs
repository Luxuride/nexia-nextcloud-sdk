//! FUSE driver implementation over [`VirtualTree`].

use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use nexia_nextcloud_sdk_vfs::{HydratedContent, HydrationState, VirtualNode, VirtualTree};

use crate::attr::{FileAttr, FileType};
use crate::entry::DirEntry;
use crate::hydrator::{AsyncHydrator, NoopHydrator};
use crate::inode::{FUSE_ROOT_INODE, InodeTable};

/// Standard POSIX error constant: No such file or directory.
pub const ENOENT: i32 = 2;
/// Standard POSIX error constant: Input/output error.
pub const EIO: i32 = 5;
/// Standard POSIX error constant: Invalid argument.
pub const EINVAL: i32 = 22;
/// Standard POSIX error constant: Not a directory.
pub const ENOTDIR: i32 = 20;
/// Standard POSIX error constant: Illegal seek (read beyond end of file).
pub const ESPIPE: i32 = 29;

/// Page size used when hydrating partial byte ranges from a remote file.
pub const HYDRATION_BLOCK_SIZE: u64 = 128 * 1024;

/// Buffered `(block_start, bytes)` windows per virtual path, for files whose full hydration failed.
type PartialBlocks = std::collections::HashMap<String, Vec<(u64, Vec<u8>)>>;

/// Core filesystem driver bridging a [`VirtualTree`] and [`InodeTable`] to FUSE operations.
pub struct FuseDriver {
    tree: Arc<RwLock<VirtualTree>>,
    inodes: Arc<RwLock<InodeTable>>,
    hydrator: Arc<dyn AsyncHydrator>,
    runtime: Option<tokio::runtime::Handle>,
    root_prefix: String,
    /// Byte windows fetched so far for files whose full hydration failed.
    ///
    /// Entries are dropped as soon as the node becomes fully hydrated again. The buffer keeps
    /// the most recent windows per path so repeated paging over one file does not re-download.
    partial_blocks: Arc<RwLock<PartialBlocks>>,
}

impl FuseDriver {
    /// Constructs a new [`FuseDriver`] over a [`VirtualTree`] with a default no-op hydrator.
    pub fn new(tree: Arc<RwLock<VirtualTree>>) -> Self {
        Self::with_hydrator(tree, Arc::new(NoopHydrator))
    }

    /// Constructs a new [`FuseDriver`] with an explicit payload hydrator.
    pub fn with_hydrator(tree: Arc<RwLock<VirtualTree>>, hydrator: Arc<dyn AsyncHydrator>) -> Self {
        Self::with_hydrator_and_root(tree, hydrator, "")
    }

    /// Constructs a new [`FuseDriver`] with an explicit payload hydrator and subtree `root_prefix`.
    ///
    /// When `root_prefix` is set (e.g. `"/InstantUpload"`), the FUSE root inode maps to
    /// this subtree in the [`VirtualTree`], allowing seamless subfolder mounting.
    pub fn with_hydrator_and_root(
        tree: Arc<RwLock<VirtualTree>>,
        hydrator: Arc<dyn AsyncHydrator>,
        root_prefix: impl AsRef<str>,
    ) -> Self {
        let raw = root_prefix.as_ref().trim();
        let normalized_root = if raw.is_empty() || raw == "/" {
            String::new()
        } else {
            let trimmed = raw.trim_end_matches('/');
            if trimmed.starts_with('/') {
                trimmed.to_string()
            } else {
                format!("/{trimmed}")
            }
        };

        let mut inodes = InodeTable::new();

        // Pre-populate inodes for existing nodes in the subtree
        if let Ok(tree_guard) = tree.read() {
            for path in tree_guard.paths() {
                if let Some(mount_path) = Self::to_mount_path_internal(path, &normalized_root)
                    && mount_path != "/"
                {
                    inodes.get_or_insert(&mount_path);
                }
            }
        }

        Self {
            tree,
            inodes: Arc::new(RwLock::new(inodes)),
            hydrator,
            runtime: tokio::runtime::Handle::try_current().ok(),
            root_prefix: normalized_root,
            partial_blocks: Arc::new(RwLock::new(PartialBlocks::new())),
        }
    }

    /// Translates a mount-relative path (e.g. `"/Camera"`) to canonical VFS path (e.g. `"/InstantUpload/Camera"`).
    pub fn to_vfs_path(&self, mount_path: &str) -> String {
        Self::to_vfs_path_internal(mount_path, &self.root_prefix)
    }

    fn to_vfs_path_internal(mount_path: &str, root_prefix: &str) -> String {
        if root_prefix.is_empty() {
            return mount_path.to_string();
        }
        if mount_path == "/" {
            return root_prefix.to_string();
        }
        let cleaned = if mount_path.starts_with('/') {
            mount_path
        } else {
            return format!("{root_prefix}/{mount_path}");
        };
        format!("{root_prefix}{cleaned}")
    }

    /// Translates a canonical VFS path (e.g. `"/InstantUpload/Camera"`) to mount-relative path (e.g. `"/Camera"`).
    ///
    /// Returns `None` if the path falls outside this driver's configured `root_prefix`.
    pub fn to_mount_path(&self, vfs_path: &str) -> Option<String> {
        Self::to_mount_path_internal(vfs_path, &self.root_prefix)
    }

    fn to_mount_path_internal(vfs_path: &str, root_prefix: &str) -> Option<String> {
        if root_prefix.is_empty() {
            return Some(vfs_path.to_string());
        }
        if vfs_path == root_prefix {
            return Some("/".to_string());
        }
        if let Some(rest) = vfs_path.strip_prefix(root_prefix)
            && rest.starts_with('/')
        {
            return Some(rest.to_string());
        }
        None
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

    pub(crate) fn execute_blocking<F>(rt: &tokio::runtime::Handle, fut: F) -> F::Output
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(cur) if cur.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| rt.block_on(fut))
            }
            Ok(_) => std::thread::scope(|s| {
                s.spawn(|| rt.block_on(fut))
                    .join()
                    .expect("hydration thread panicked")
            }),
            Err(_) => rt.block_on(fut),
        }
    }

    fn ensure_directory_hydrated(&self, vfs_path: &str) {
        let needs_hydrate = {
            if let Ok(tree) = self.tree.read() {
                if let Some(node) = tree.get(vfs_path) {
                    node.is_directory() && !node.is_populated
                } else {
                    false
                }
            } else {
                false
            }
        };

        if needs_hydrate {
            tracing::debug!(vfs_path, "FUSE triggering directory hydration");
            let fut = self.hydrator.hydrate_directory(vfs_path);
            if let Some(rt) = self.runtime_handle() {
                let _ = Self::execute_blocking(&rt, fut);
            } else if let Ok(rt) = tokio::runtime::Handle::try_current() {
                let _ = Self::execute_blocking(&rt, fut);
            }
        }
    }

    /// Looks up a child node by `name` inside the parent directory identified by `parent_ino`.
    pub fn lookup(&self, parent_ino: u64, name: &str) -> Result<FileAttr, i32> {
        tracing::trace!(parent_ino, name, "FUSE lookup");
        let parent_mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(parent_ino).ok_or(ENOENT)?.to_string()
        };

        let child_mount_path = if parent_mount_path == "/" {
            format!("/{name}")
        } else {
            format!("{parent_mount_path}/{name}")
        };

        let parent_vfs_path = self.to_vfs_path(&parent_mount_path);
        let child_vfs_path = self.to_vfs_path(&child_mount_path);

        // If child is not in tree, ensure parent directory is populated
        {
            let needs_parent_hydrated = {
                let tree = self.tree.read().map_err(|_| EINVAL)?;
                tree.get(&child_vfs_path).is_none()
            };
            if needs_parent_hydrated {
                self.ensure_directory_hydrated(&parent_vfs_path);
            }
        }

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&child_vfs_path).ok_or(ENOENT)?;

        let mut inodes = self.inodes.write().map_err(|_| EINVAL)?;
        let ino = inodes.get_or_insert(&child_mount_path);

        Ok(Self::node_to_attr(ino, node))
    }

    /// Retrieves attributes for the node identified by `ino`.
    pub fn getattr(&self, ino: u64) -> Result<FileAttr, i32> {
        let mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        let vfs_path = self.to_vfs_path(&mount_path);
        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&vfs_path).ok_or(ENOENT)?;

        Ok(Self::node_to_attr(ino, node))
    }

    /// Checks whether the file node for `ino` is an unhydrated placeholder.
    pub fn is_placeholder(&self, ino: u64) -> bool {
        let mount_path = {
            let inodes = match self.inodes.read() {
                Ok(guard) => guard,
                Err(_) => return false,
            };
            match inodes.path_by_inode(ino) {
                Some(p) => p.to_string(),
                None => return false,
            }
        };

        let vfs_path = self.to_vfs_path(&mount_path);
        let tree = match self.tree.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        tree.get(&vfs_path)
            .map(|n| n.is_placeholder())
            .unwrap_or(false)
    }

    /// Reads directory child entries for directory `ino` starting from `offset`.
    pub fn readdir(&self, ino: u64, offset: i64) -> Result<Vec<DirEntry>, i32> {
        let mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };

        let vfs_path = self.to_vfs_path(&mount_path);
        self.ensure_directory_hydrated(&vfs_path);

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&vfs_path).ok_or(ENOENT)?;

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
            let parent_mount_path = nexia_nextcloud_sdk_vfs::parent_path(&mount_path)
                .unwrap_or_else(|| "/".to_string());
            let mut inodes = self.inodes.write().map_err(|_| EINVAL)?;
            inodes.get_or_insert(&parent_mount_path)
        };
        entries.push(DirEntry::new(parent_ino, 2, FileType::Directory, ".."));

        // 3. Children
        self.hydrator.on_directory_read(&vfs_path);
        let children = tree.children(&vfs_path);

        let mut inodes_guard = self.inodes.write().map_err(|_| EINVAL)?;
        let mut preview_candidates = Vec::new();
        let mut cur_offset = 3;
        for child in children {
            let child_mount_path = match self.to_mount_path(child.path()) {
                Some(p) => p,
                None => continue,
            };
            let child_ino = inodes_guard.get_or_insert(&child_mount_path);
            let kind = if child.is_directory() {
                FileType::Directory
            } else {
                if child.metadata.has_preview || is_preview_name(child.name()) {
                    preview_candidates.push(child.path().to_string());
                }
                FileType::RegularFile
            };
            entries.push(DirEntry::new(child_ino, cur_offset, kind, child.name()));
            cur_offset += 1;
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
    /// Placeholders are transparently hydrated: a *full* hydration is attempted first so the
    /// whole file becomes readable, and the read then falls back to a ranged hydration when the
    /// full fetch fails. A node that is already hydrated through a local cache path
    /// ([`HydratedContent::Path`]) is read straight from that file — it is never answered with a
    /// silent short read.
    pub async fn read(&self, ino: u64, offset: i64, size: u32) -> Result<Vec<u8>, i32> {
        let mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };
        let vfs_path = self.to_vfs_path(&mount_path);
        tracing::trace!(ino, mount_path = %mount_path, vfs_path = %vfs_path, offset, size, "FUSE read request");

        let (is_placeholder, file_size) = {
            let tree = self.tree.read().map_err(|_| EINVAL)?;
            let node = tree.get(&vfs_path).ok_or(ENOENT)?;
            (node.is_placeholder(), node.size())
        };

        if is_placeholder {
            tracing::info!(ino, vfs_path = %vfs_path, "FUSE triggering on-demand file hydration");
            match self.hydrator.hydrate(&vfs_path).await {
                Ok(bytes) => {
                    let mut tree = self.tree.write().map_err(|_| EINVAL)?;
                    tree.hydrate(&vfs_path, bytes).map_err(|_| EINVAL)?;
                    self.clear_partial_ranges(&vfs_path);
                }
                Err(err) => {
                    tracing::warn!(vfs_path = %vfs_path, error = %err, "full hydration failed, falling back to ranged read");
                    return self.read_ranged(&vfs_path, file_size, offset, size).await;
                }
            }
        }

        // Fast path: fully hydrated in memory.
        let cached_path = {
            let tree = self.tree.read().map_err(|_| EINVAL)?;
            let node = tree.get(&vfs_path).ok_or(ENOENT)?;
            match node.hydration() {
                HydrationState::Hydrated(HydratedContent::Bytes(bytes)) => {
                    let bytes = bytes.clone();
                    drop(tree);
                    return Ok(Self::slice_from_memory(&bytes, offset, size));
                }
                HydrationState::Hydrated(HydratedContent::Path(path)) => Some(path.clone()),
                HydrationState::Placeholder => None,
            }
        };

        // Hydrated through a local cache file: serve the range from disk.
        if let Some(path) = cached_path {
            return Self::read_local_range(&path, offset, size).await;
        }

        // Partially hydrated (or hydration state vanished under us): merge ranges.
        self.read_ranged(&vfs_path, file_size, offset, size).await
    }

    /// Slices a memory buffer, clamping the requested range to the buffer bounds.
    fn slice_from_memory(bytes: &[u8], offset: i64, size: u32) -> Vec<u8> {
        let off = (offset.max(0) as usize).min(bytes.len());
        let end = off.saturating_add(size as usize).min(bytes.len());
        bytes[off..end].to_vec()
    }

    /// Serves a read range from a hydrated local cache file.
    async fn read_local_range(
        path: &std::path::Path,
        offset: i64,
        size: u32,
    ) -> Result<Vec<u8>, i32> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        if offset < 0 {
            return Err(EINVAL);
        }
        let mut file = tokio::fs::File::open(path).await.map_err(|err| {
            tracing::error!(path = %path.display(), error = %err, "failed to open hydrated cache file");
            EIO
        })?;
        let len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
        let start = (offset as u64).min(len);
        if start >= len {
            return Ok(Vec::new());
        }
        let want = (size as u64).min(len - start);
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|_| EIO)?;
        let mut buf = vec![0u8; want as usize];
        file.read_exact(&mut buf).await.map_err(|err| {
            tracing::error!(path = %path.display(), error = %err, "failed to read hydrated cache file");
            EIO
        })?;
        Ok(buf)
    }

    /// Fetches (or serves from) a byte range of a not-fully-hydrated file.
    async fn read_ranged(
        &self,
        vfs_path: &str,
        file_size: u64,
        offset: i64,
        size: u32,
    ) -> Result<Vec<u8>, i32> {
        if offset < 0 {
            return Err(EINVAL);
        }
        let start = offset as u64;
        if file_size > 0 && start >= file_size {
            return Ok(Vec::new());
        }

        let known = match &file_size {
            0 => size as u64,
            total => (*total).saturating_sub(start).min(size as u64),
        };
        let mut out = vec![0u8; known as usize];
        let mut filled = 0usize;

        while filled < out.len() {
            let cursor = start + filled as u64;

            // Serve from an earlier fetched block when it already covers this position.
            if let Some((chunk_start, chunk)) = self.take_cached_block(vfs_path, cursor) {
                let skip = (cursor - chunk_start) as usize;
                let n = (chunk.len() - skip).min(out.len() - filled);
                out[filled..filled + n].copy_from_slice(&chunk[skip..skip + n]);
                filled += n;
                continue;
            }

            let block_start = (cursor / HYDRATION_BLOCK_SIZE) * HYDRATION_BLOCK_SIZE;
            let mut block_len = 2 * HYDRATION_BLOCK_SIZE;
            if file_size > 0 {
                let remaining = file_size.saturating_sub(block_start);
                if remaining == 0 {
                    break;
                }
                block_len = block_len.min(remaining).max(1);
            }

            let Some(bytes) = self
                .fetch_range(vfs_path, block_start, block_len as u32)
                .await?
            else {
                // Nothing was fetched. Answering with a short/empty read here would look like
                // end-of-file and get cached as such by the kernel, so a transient network error
                // would permanently truncate the file. Fail the request instead; the reader
                // retries and hydration has another chance to succeed.
                return Err(EIO);
            };
            if bytes.is_empty() {
                return Err(EIO);
            }

            let short = (bytes.len() as u64) < block_len;
            self.store_block(vfs_path, block_start, bytes.clone());

            let skip = (cursor - block_start) as usize;
            if skip < bytes.len() {
                let n = (bytes.len() - skip).min(out.len() - filled);
                out[filled..filled + n].copy_from_slice(&bytes[skip..skip + n]);
                filled += n;
            }

            // A short block read means we reached the end of the remote file.
            if short {
                break;
            }
        }

        out.truncate(filled);
        Ok(out)
    }

    /// Calls the hydrator for `len` bytes at `start`, returning `None` when it fails.
    async fn fetch_range(
        &self,
        vfs_path: &str,
        start: u64,
        len: u32,
    ) -> Result<Option<Vec<u8>>, i32> {
        match self.hydrator.hydrate_range(vfs_path, start, len).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) => {
                tracing::warn!(vfs_path, start, len, error = %err, "ranged hydration failed");
                Ok(None)
            }
        }
    }

    /// Returns the buffered block that already covers `cursor`, as `(block_start, bytes)`.
    fn take_cached_block(&self, vfs_path: &str, cursor: u64) -> Option<(u64, Vec<u8>)> {
        let blocks = self.partial_blocks.read().ok()?;
        let entries = blocks.get(vfs_path)?;
        entries
            .iter()
            .find(|(start, bytes)| cursor >= *start && cursor < *start + bytes.len() as u64)
            .map(|(start, bytes)| (*start, bytes.clone()))
    }

    /// Buffers a fetched block so subsequent reads of the same window stay local.
    fn store_block(&self, vfs_path: &str, start: u64, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        let Ok(mut blocks) = self.partial_blocks.write() else {
            return;
        };
        let entry = blocks.entry(vfs_path.to_string()).or_default();
        entry.push((start, bytes));
        // Keep the per-file block cache bounded (most recent 16 blocks win).
        if entry.len() > 16 {
            entry.sort_by_key(|(s, _)| *s);
            entry.drain(..entry.len() - 16);
        }
    }

    /// Drops all buffered partial blocks for `vfs_path` (called once it is fully hydrated).
    fn clear_partial_ranges(&self, vfs_path: &str) {
        if let Ok(mut blocks) = self.partial_blocks.write() {
            blocks.remove(vfs_path);
        }
    }

    /// Retrieves extended attributes such as thumbnails (`user.thumb.{width}x{height}`).
    pub fn getxattr(&self, ino: u64, name: &str) -> Result<Vec<u8>, i32> {
        let mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };
        let vfs_path = self.to_vfs_path(&mount_path);

        if let Some(dim_str) = name.strip_prefix("user.thumb.")
            && let Some((w_str, h_str)) = dim_str.split_once('x')
            && let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>())
        {
            let tree = self.tree.read().map_err(|_| EINVAL)?;
            if let Some(bytes) = tree.get_thumbnail(&vfs_path, w, h) {
                return Ok(bytes.to_vec());
            }
        }

        Err(ENOENT)
    }

    /// Lists supported extended attribute names for `ino`.
    pub fn listxattr(&self, ino: u64) -> Result<Vec<String>, i32> {
        let mount_path = {
            let inodes = self.inodes.read().map_err(|_| EINVAL)?;
            inodes.path_by_inode(ino).ok_or(ENOENT)?.to_string()
        };
        let vfs_path = self.to_vfs_path(&mount_path);

        let tree = self.tree.read().map_err(|_| EINVAL)?;
        let node = tree.get(&vfs_path).ok_or(ENOENT)?;

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

/// Parses a date string (HTTP-date RFC 1123, ISO 8601, or raw UNIX seconds) into UNIX epoch seconds.
#[must_use]
pub fn parse_date_to_unix_secs(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(secs);
    }
    if let Some(secs) = parse_http_date(trimmed) {
        return Some(secs);
    }
    parse_iso_date(trimmed)
}

fn parse_node_mtime(s: Option<&str>) -> SystemTime {
    let secs = s.and_then(parse_date_to_unix_secs).unwrap_or(1_700_000_000);
    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
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
    use nexia_nextcloud_sdk_vfs::{NodeMetadata, VfsError};
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

    #[test]
    fn subtree_prefix_mapping() {
        let mut tree = VirtualTree::new("root");
        let dir_meta = NodeMetadata::directory("/InstantUpload");
        tree.insert(VirtualNode::new_directory(dir_meta));

        let camera_meta = NodeMetadata::directory("/InstantUpload/Camera");
        tree.insert(VirtualNode::new_directory(camera_meta));

        let file_meta = NodeMetadata::file("/InstantUpload/Camera/photo.jpg").with_size(100);
        tree.insert(VirtualNode::new_file(file_meta));

        let other_meta = NodeMetadata::file("/Other/doc.txt").with_size(50);
        tree.insert(VirtualNode::new_file(other_meta));

        let driver = FuseDriver::with_hydrator_and_root(
            Arc::new(RwLock::new(tree)),
            Arc::new(TestHydrator),
            "/InstantUpload",
        );

        assert_eq!(driver.to_vfs_path("/"), "/InstantUpload");
        assert_eq!(driver.to_vfs_path("/Camera"), "/InstantUpload/Camera");
        assert_eq!(
            driver.to_vfs_path("/Camera/photo.jpg"),
            "/InstantUpload/Camera/photo.jpg"
        );

        assert_eq!(
            driver.to_mount_path("/InstantUpload"),
            Some("/".to_string())
        );
        assert_eq!(
            driver.to_mount_path("/InstantUpload/Camera"),
            Some("/Camera".to_string())
        );
        assert_eq!(driver.to_mount_path("/Other/doc.txt"), None);

        let root_attr = driver.getattr(FUSE_ROOT_INODE).unwrap();
        assert!(root_attr.is_dir());

        let camera_attr = driver.lookup(FUSE_ROOT_INODE, "Camera").unwrap();
        assert!(camera_attr.is_dir());

        let photo_attr = driver.lookup(camera_attr.ino, "photo.jpg").unwrap();
        assert!(photo_attr.is_file());
        assert_eq!(photo_attr.size, 100);
    }

    struct PopulatingHydrator {
        tree: Arc<RwLock<VirtualTree>>,
    }

    impl AsyncHydrator for PopulatingHydrator {
        fn hydrate(
            &self,
            _canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            Box::pin(async { Ok(b"data".to_vec()) })
        }

        fn hydrate_directory(
            &self,
            canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<(), VfsError>> + Send + '_>> {
            let tree = Arc::clone(&self.tree);
            let path = canonical_path.to_string();
            Box::pin(async move {
                let mut guard = tree
                    .write()
                    .map_err(|_| VfsError::InvalidPath("lock poisoned".to_string()))?;
                if let Some(node) = guard.get_mut(&path) {
                    node.is_populated = true;
                }
                guard.insert(VirtualNode::new_file(
                    NodeMetadata::file(format!("{path}/fetched.txt")).with_size(10),
                ));
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn directory_hydration_on_readdir_and_lookup() {
        let tree = Arc::new(RwLock::new(VirtualTree::new("root")));
        let hydrator = Arc::new(PopulatingHydrator {
            tree: Arc::clone(&tree),
        });

        let driver = FuseDriver::with_hydrator(Arc::clone(&tree), hydrator);

        // Before readdir, root is unpopulated and has no fetched.txt
        let entries = driver.readdir(FUSE_ROOT_INODE, 0).unwrap();
        assert_eq!(entries.len(), 3); // ., .., fetched.txt
        assert_eq!(entries[2].name, "fetched.txt");

        // Now lookup should find fetched.txt
        let attr = driver.lookup(FUSE_ROOT_INODE, "fetched.txt").unwrap();
        assert!(attr.is_file());
        assert_eq!(attr.size, 10);
    }

    /// Regression: a node hydrated through a local cache file must serve bytes from that file.
    ///
    /// It used to fall through to `Ok(Vec::new())`, which the kernel cached as end-of-file: the
    /// image looked readable but empty, and only a client restart appeared to fix it.
    #[tokio::test]
    async fn path_backed_hydration_serves_bytes_from_cache_file() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        let dir = std::env::temp_dir().join(format!("nexia-fuse-path-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cached = dir.join("photo.jpg");
        std::fs::write(&cached, &payload).unwrap();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/photo.jpg").with_size(payload.len() as u64),
        ));
        tree.hydrate_path("/photo.jpg", cached.clone()).unwrap();

        // A hydrator that must never be reached: the payload is already local.
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), Arc::new(NoopHydrator));
        let attr = driver.lookup(FUSE_ROOT_INODE, "photo.jpg").unwrap();

        assert_eq!(driver.read(attr.ino, 0, 512).await.unwrap(), payload[..512]);
        assert_eq!(
            driver.read(attr.ino, 1024, 256).await.unwrap(),
            payload[1024..1280]
        );
        // Past the end of the file is a legitimate empty read, not an error.
        assert!(driver.read(attr.ino, 4096, 128).await.unwrap().is_empty());

        let _ = std::fs::remove_file(&cached);
        let _ = std::fs::remove_dir(&dir);
    }

    /// Records what `hydrate_range` was asked for and serves from an in-memory buffer.
    struct RangeHydrator {
        payload: Vec<u8>,
        full_fails: bool,
        range_calls: Arc<RwLock<Vec<(u64, u32)>>>,
    }

    impl AsyncHydrator for RangeHydrator {
        fn hydrate(
            &self,
            canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            let fail = self.full_fails;
            let path = canonical_path.to_string();
            Box::pin(async move {
                if fail {
                    Err(VfsError::NotFound(path))
                } else {
                    Ok(Vec::new())
                }
            })
        }

        fn hydrate_range(
            &self,
            _canonical_path: &str,
            offset: u64,
            size: u32,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            self.range_calls.write().unwrap().push((offset, size));
            let start = (offset as usize).min(self.payload.len());
            let end = (start + size as usize).min(self.payload.len());
            let slice = self.payload[start..end].to_vec();
            Box::pin(async move { Ok(slice) })
        }
    }

    /// A failed full hydration must fall back to ranged hydration rather than failing the read.
    #[tokio::test]
    async fn read_falls_back_to_ranged_hydration() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(512 * 1024).collect();
        let calls = Arc::new(RwLock::new(Vec::new()));
        let hydrator = Arc::new(RangeHydrator {
            payload: payload.clone(),
            full_fails: true,
            range_calls: calls.clone(),
        });

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/big.bin").with_size(payload.len() as u64),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "big.bin").unwrap();

        // A read straddling two 128 KiB hydration blocks must stitch them together.
        let offset = HYDRATION_BLOCK_SIZE as usize - 1024;
        let got = driver.read(attr.ino, offset as i64, 4096).await.unwrap();
        assert_eq!(got, payload[offset..offset + 4096]);

        // The second read inside an already fetched block must not hit the network again.
        let calls_before = calls.read().unwrap().len();
        let again = driver.read(attr.ino, offset as i64, 4096).await.unwrap();
        assert_eq!(again, got);
        assert_eq!(
            calls.read().unwrap().len(),
            calls_before,
            "a buffered block must be reused instead of refetched"
        );

        // The node stays a placeholder: ranged reads do not pretend to be a full hydration.
        assert!(driver.read(attr.ino, 0, 16).await.is_ok());
    }

    /// A failing fetch must surface as `EIO`, never as a short read.
    ///
    /// An empty buffer is indistinguishable from end-of-file, and the kernel caches it against the
    /// inode, so a transient network error would permanently truncate the file.
    #[tokio::test]
    async fn failed_hydration_reports_eio_not_eof() {
        let calls = Arc::new(RwLock::new(Vec::new()));
        let hydrator = Arc::new(FailingRangeHydrator { calls });

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/broken.bin").with_size(10_000),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "broken.bin").unwrap();

        assert_eq!(driver.read(attr.ino, 0, 4096).await, Err(EIO));
    }

    /// Hydrator whose full and ranged fetches both fail.
    struct FailingRangeHydrator {
        calls: Arc<RwLock<Vec<(u64, u32)>>>,
    }

    impl AsyncHydrator for FailingRangeHydrator {
        fn hydrate(
            &self,
            canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            let err = VfsError::NotFound(canonical_path.to_string());
            Box::pin(async move { Err(err) })
        }

        fn hydrate_range(
            &self,
            canonical_path: &str,
            offset: u64,
            size: u32,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            self.calls.write().unwrap().push((offset, size));
            let err = VfsError::NotFound(canonical_path.to_string());
            Box::pin(async move { Err(err) })
        }
    }
}
