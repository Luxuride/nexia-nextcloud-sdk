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

/// How much is fetched in one ranged request, and the alignment of the windows.
///
/// Each round trip has a fixed cost that dominates the transfer for a small window: measured
/// against a real server a 256 KiB request took ~0.4-0.6 s wall clock while a full 4 MiB file read
/// of 16 such requests took ~8 s, i.e. almost all of it latency rather than bandwidth. Fetching
/// four blocks at a time cuts the request count by four for no extra data. Windows are aligned to
/// their own size so consecutive windows are disjoint — aligning to a single block instead makes
/// each window overlap the previous one and re-fetch up to three blocks of already cached data.
pub const RANGED_WINDOW_SIZE: u64 = 4 * HYDRATION_BLOCK_SIZE;

/// Largest file that a placeholder read will fetch in one piece.
///
/// This is exactly one ranged window, which makes the rule self-evident rather than a guess: for a
/// file this small, a full hydration transfers no more data than the single ranged fetch it
/// replaces, and it leaves the whole file readable so no further round trips are needed. Above it,
/// hydrating wholesale to answer one small read blocks its FUSE thread for a transfer that can be
/// tens of MiB — measured at 46-620 KiB/s against a real server, so tens of seconds — and caches
/// the entire payload in the page cache. Those reads go through
/// `FuseDriver::read_ranged` instead.
pub const FULL_HYDRATION_MAX_SIZE: u64 = RANGED_WINDOW_SIZE;

/// How many windows of readahead a sequential reader gets after the window it just touched.
///
/// Measured against a real server, a range request costs a fixed ~300 ms plus ~0.1 ms/KiB, so the
/// first bytes of *any* request are four orders of magnitude more expensive than the marginal byte.
/// A reader that touches window `n` and then window `n + 1` pays that fixed cost again for data it
/// could have had almost free. Prefetching a few windows ahead converts a page-by-page walk into
/// one paid round trip per group of windows.
///
/// Four is a deliberate ceiling: `FuseDriver::store_block` keeps only the most recent 16 blocks
/// per file, so prefetching much further would evict the very data it just fetched, and each
/// prefetch also competes for the download gate with real reads.
pub const READAHEAD_WINDOWS: u64 = 4;

/// Largest single range request issued to satisfy one read.
///
/// The cost of a range request is dominated by its round trip, not its size: measured against a
/// real server each hop takes ~0.5 s whether it carries 256 KiB or several MiB. Since a caller that
/// asks for 9 MiB has already accepted 9 MiB of latency and memory, fetching it as one request is
/// strictly better than fetching it as 18 windows one after another — the same bytes, one round
/// trip instead of eighteen. This cap exists only to keep a single request from growing without
/// bound if a caller ever asks for an enormous read; it is far above the largest read a real image
/// loader issues (9.2 MiB observed), so in practice a read is served by one request.
pub const MAX_RANGE_REQUEST_SIZE: u64 = 16 * 1024 * 1024;

/// Merges the windows covering `len` bytes at `start` into contiguous fetch spans.
///
/// Returns `(offset, length)` pairs aligned so the first span starts on the window containing
/// `start`, which keeps the cached-block layout stable for later partial reads. Adjacent windows
/// are deliberately *not* kept separate: fetching them individually is what made a single large
/// read take one round trip per window.
fn plan_fetch_spans(start: u64, len: u64, file_size: u64) -> Vec<(u64, u64)> {
    let mut spans = Vec::new();
    if len == 0 {
        return spans;
    }
    let want_end = start.saturating_add(len);
    let mut cursor = (start / RANGED_WINDOW_SIZE) * RANGED_WINDOW_SIZE;
    // Round the end *up* to a window boundary. Ending exactly at the requested byte would turn a
    // sequential reader (a 16 KiB read repeated across a file) into one round trip per read; the
    // whole point of window-aligned prefetch is that the rest of the window is already buffered
    // when the next read arrives.
    let rounded_end = want_end
        .div_ceil(RANGED_WINDOW_SIZE)
        .saturating_mul(RANGED_WINDOW_SIZE);
    let end = rounded_end.min(if file_size > 0 {
        file_size
    } else {
        rounded_end
    });
    while cursor < end {
        let span_len = (end - cursor).min(MAX_RANGE_REQUEST_SIZE);
        spans.push((cursor, span_len));
        cursor += span_len;
    }
    spans
}

/// Upper bound on the number of FUSE threads that may sit blocked on a remote fetch at once.
///
/// Every read of an unhydrated placeholder occupies its FUSE thread for the whole round trip,
/// which is hundreds of milliseconds to seconds here. A directory of photos makes a file manager
/// touch many files at once, so without a bound the entire thread pool ends up waiting on the
/// network and *unrelated* requests — `getattr`, `readdir`, `lookup`, an application's own probe —
/// cannot be served at all. A measured 60-way concurrent read of a 625-file directory pushed
/// `ls` latency from 4 ms to 9.4 s that way: the mount looks frozen while nothing is stuck.
///
/// Holding the gate below the thread-pool size keeps workers free for metadata, so listings stay
/// responsive while downloads queue. Deferring the surplus reads to the thumbnailers is done
/// separately via `ECHILD`, since those reads are speculative by nature.
pub const MAX_INFLIGHT_DOWNLOADS: usize = 8;

/// Upper bound on buffered fetch spans kept per file for partial reads.
///
/// A span can be as large as [`MAX_RANGE_REQUEST_SIZE`], so this cap is also a memory bound per
/// file: without it a long-lived viewer accumulates one buffer per read until the mount is
/// restarted.
const MAX_CACHED_BLOCKS_PER_FILE: usize = 16;

/// Upper bound on files with buffered fetch spans.
///
/// Entries are dropped when a file is fully hydrated, but this mount has no `forget`/`unlink`
/// hook, so a deleted file would otherwise keep its buffers for the lifetime of the mount.
const MAX_CACHED_FILES: usize = 512;

/// Bounds how many FUSE threads may be blocked on a remote payload fetch simultaneously.
///
/// Deliberately a plain mutex/condvar gate with a blocking acquire: the callers are `fuser`'s
/// synchronous event-loop threads, not tasks, so there is no async context to await in, and a
/// semaphore shared with the tokio runtime could not be waited on without one.
#[derive(Debug)]
pub struct DownloadGate {
    limit: usize,
    in_flight: std::sync::Mutex<usize>,
    released: std::sync::Condvar,
}

impl DownloadGate {
    /// Creates a gate allowing `limit` concurrent downloads (at least one).
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            in_flight: std::sync::Mutex::new(0),
            released: std::sync::Condvar::new(),
        }
    }

    /// The configured concurrency limit.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Number of downloads currently holding a slot.
    pub fn in_flight(&self) -> usize {
        self.in_flight.lock().map(|g| *g).unwrap_or(0)
    }

    /// How many downloads can still start without waiting for a slot.
    ///
    /// A poisoned mutex reports "plenty" rather than "none": callers use this only to decide
    /// whether to refuse, and refusing a read because of a poisoned lock would be worse than
    /// reading without the throttle.
    pub fn available(&self) -> usize {
        match self.in_flight.lock() {
            Ok(in_flight) => self.limit.saturating_sub(*in_flight),
            Err(_) => usize::MAX,
        }
    }

    /// `true` when starting a download would park the calling thread.
    ///
    /// Every FUSE worker comes out of one shared pool, so a read that waits here holds the very
    /// thread the *next* `getattr`/`lookup` needs — which is how one window enumerating a
    /// directory used to make every other window choppy at the same time.
    pub fn is_saturated(&self) -> bool {
        self.available() == 0
    }

    /// Blocks until a download slot is free, returning a guard that frees it on drop.
    ///
    /// A poisoned mutex is treated as "no limit" rather than as an error: refusing to read at all
    /// would be a worse failure than reading without the throttle.
    pub fn acquire(self: &Arc<Self>) -> Option<DownloadGuard> {
        let mut in_flight = self.in_flight.lock().ok()?;
        while *in_flight >= self.limit {
            in_flight = self.released.wait(in_flight).ok()?;
        }
        *in_flight += 1;
        Some(DownloadGuard {
            gate: Arc::clone(self),
        })
    }
}

/// RAII slot returned by [`DownloadGate::acquire`]; releases the slot when dropped.
#[derive(Debug)]
pub struct DownloadGuard {
    gate: Arc<DownloadGate>,
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = self.gate.in_flight.lock() {
            *in_flight = in_flight.saturating_sub(1);
        }
        self.gate.released.notify_one();
    }
}

/// Range requests a single read may split its span across.
///
/// One range request is served at roughly 1.4 MB/s, but two concurrent requests raised the
/// aggregate to ~2 MB/s on the same link, so a large transfer finishes sooner when it is fetched
/// as a few parallel requests instead of one long one. The count is deliberately small: every extra
/// request pays the fixed per-request cost again, which is why small spans are left alone.
pub const MAX_PARALLEL_RANGES: usize = 4;

/// Transfers below this size are fetched as one request; splitting only pays off above it.
const PARALLEL_SPLIT_MIN: u64 = 2 * RANGED_WINDOW_SIZE;

/// Buffered `(block_start, bytes)` windows per virtual path, for files whose full hydration failed.
type PartialBlocks = std::collections::HashMap<String, Vec<(u64, Vec<u8>)>>;

/// One in-flight range request, shared by every reader that asked for the same bytes.
type InflightRange = std::sync::Arc<tokio::sync::OnceCell<Option<Vec<u8>>>>;

/// Identifies a range request: virtual path, start offset and length.
type InflightKey = (String, u64, u32);

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
    /// Bounds how many threads may wait on a remote fetch, protecting metadata requests.
    download_gate: Arc<DownloadGate>,
    /// Where readahead last reached, per file: `(end_of_requested_span, end_of_prefetched)`.
    ///
    /// A reader is treated as sequential only when its next access begins within the range
    /// prefetched for it, which is the only evidence those prefetched bytes will be consumed.
    /// Without that check every first touch of every file prefetches four extra windows, and a file
    /// manager building thumbnails for hundreds of photos pays for megabytes it will never read.
    readahead_cursor: Arc<RwLock<std::collections::HashMap<String, (u64, u64)>>>,
    /// Range requests currently in flight, keyed by path, start and length.
    ///
    /// A cold sequential read is a pipeline: the kernel issues overlapping reads, so two FUSE
    /// threads ask for the same window at the same moment. Both used to fetch it independently,
    /// which doubled both the latency and the transferred bytes of opening a cold file (measured:
    /// every window after the first was downloaded twice). Joining the flight that is already
    /// running makes the second reader wait for those bytes instead of paying for them again.
    inflight_ranges: Arc<std::sync::Mutex<std::collections::HashMap<InflightKey, InflightRange>>>,
    /// Distinguishes a file manager browsing a directory from a real bulk read.
    ///
    /// A directory sweep touches many files with small speculative reads, each costing a full
    /// round trip; a copy keeps reading one file. See [`crate::intent`].
    read_intent: Arc<crate::intent::ReadIntent>,
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
            download_gate: Arc::new(DownloadGate::new(MAX_INFLIGHT_DOWNLOADS)),
            readahead_cursor: Arc::new(RwLock::new(std::collections::HashMap::new())),
            inflight_ranges: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            read_intent: Arc::new(crate::intent::ReadIntent::default()),
        }
    }

    /// Classifies a read as browsing or streaming, so `fs::read` can refuse speculative sweeps.
    ///
    /// Decides whether this read belongs to a file manager's directory sweep.
    ///
    /// Returns [`crate::intent::ReadVerdict::Allow`] for every process that is not one of the file
    /// managers in [`crate::intent::BROWSER_COMMS`], so copies, archivers, viewers and scripts are
    /// never throttled. The process name is read from `/proc/{pid}/comm`, the one name a process
    /// cannot forge.
    ///
    /// `pid` is the calling process; the inode is the file identity, which is stable for the
    /// lifetime of the mount and needs no path resolution here.
    pub fn classify_read(
        &self,
        pid: u32,
        ino: u64,
        offset: u64,
        size: u32,
    ) -> crate::intent::ReadVerdict {
        let browser = crate::intent::is_browser_process(pid);
        self.read_intent.classify(pid, browser, ino, offset, size)
    }

    /// The gate bounding simultaneous remote fetches.
    pub fn download_gate(&self) -> &Arc<DownloadGate> {
        &self.download_gate
    }

    /// Sets the concurrent-download limit, used to keep it below the FUSE thread-pool size.
    pub fn set_download_limit(&mut self, limit: usize) {
        self.download_gate = Arc::new(DownloadGate::new(limit));
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

    /// Runs `fut` to completion from a synchronous FUSE thread.
    ///
    /// Returns `Err(EIO)` when the future panicked, so a failing hydration is reported to the
    /// caller as an I/O error instead of unwinding through the dispatch loop.
    pub(crate) fn execute_blocking<F>(rt: &tokio::runtime::Handle, fut: F) -> Result<F::Output, i32>
    where
        F: std::future::Future + Send,
        F::Output: Send,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(cur) if cur.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                Ok(tokio::task::block_in_place(|| rt.block_on(fut)))
            }
            Ok(_) => {
                // A current-thread runtime cannot be re-entered, so the future runs on a scratch
                // thread. A panic in a hydrator must surface as a failed request rather than as a
                // panic on the FUSE dispatch path, which would take the mount down.
                match std::thread::scope(|scope| scope.spawn(|| rt.block_on(fut)).join()) {
                    Ok(value) => Ok(value),
                    Err(panic) => {
                        let reason = panic
                            .downcast_ref::<&str>()
                            .map(|message| (*message).to_string())
                            .or_else(|| panic.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "non-string panic payload".to_string());
                        tracing::error!(reason, "blocking future panicked");
                        Err(EIO)
                    }
                }
            }
            Err(_) => Ok(rt.block_on(fut)),
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
            let outcome = if let Some(rt) = self.runtime_handle() {
                Self::execute_blocking(&rt, fut)
            } else if let Ok(rt) = tokio::runtime::Handle::try_current() {
                Self::execute_blocking(&rt, fut)
            } else {
                return;
            };
            if let Err(code) = outcome {
                tracing::warn!(vfs_path, error_code = code, "directory hydration failed");
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
            // Take a download slot before touching the network, for the whole read: a burst of
            // reads must not occupy every FUSE thread, or `getattr`/`readdir`/`lookup` cannot be
            // served and the mount appears frozen even though nothing is stuck.
            let _download_slot = self.download_gate.acquire();
            // Only small files are fetched whole. A full hydration ties up this FUSE thread for the
            // entire transfer, so doing it for every touch is what made directory listings (and
            // therefore file managers) hang: one 35 MiB photo held a thread for 47 s and grew RSS by
            // 274 MiB to serve a 4 KiB read, and the page cache keeps every byte afterwards. Large
            // files go straight to the ranged path, which fetches only the windows actually read.
            if file_size > 0 && file_size <= FULL_HYDRATION_MAX_SIZE {
                tracing::info!(
                    ino,
                    vfs_path = %vfs_path,
                    file_size,
                    "FUSE triggering on-demand full file hydration"
                );
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
            } else {
                tracing::debug!(
                    ino,
                    vfs_path = %vfs_path,
                    file_size,
                    offset,
                    size,
                    "Serving large placeholder via ranged hydration"
                );
                return self.read_ranged(&vfs_path, file_size, offset, size).await;
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

        // Hydrated through a local cache file: serve the range from disk. A node can point at a file
        // that no longer exists — the cache may be cleared or deleted behind our back, and the tree
        // now persists hydration as a path — so an unreadable cache file falls back to hydrating
        // again instead of failing every read for the rest of the session.
        if let Some(path) = cached_path {
            match Self::read_local_range(&path, offset, size).await {
                Ok(bytes) => return Ok(bytes),
                Err(err) if err == EIO => {
                    tracing::warn!(
                        vfs_path = %vfs_path,
                        cached = %path.display(),
                        "hydrated cache file is unreadable, hydrating again"
                    );
                    if let Ok(mut tree) = self.tree.write() {
                        let _ = tree.dehydrate(&vfs_path);
                    }
                    // This path bypasses the placeholder branch above, so it has to take the
                    // download slot itself; `read_ranged` never acquires one (taking a second slot
                    // while holding one deadlocks the gate).
                    let _download_slot = self.download_gate.acquire();
                    return self.read_ranged(&vfs_path, file_size, offset, size).await;
                }
                Err(err) => return Err(err),
            }
        }

        // Partially hydrated (or hydration state vanished under us): merge ranges.
        let _download_slot = self.download_gate.acquire();
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

        // Serve whatever is already buffered, then fetch the rest as few contiguous spans.
        //
        // The loop deliberately merges adjacent windows into one request. Walking window by window
        // and awaiting each fetch put one round trip per window in series on a single FUSE thread:
        // a live session issued reads of 8.8-9.2 MiB, i.e. 18 windows at ~0.5 s per hop, which is
        // what a file manager experiences as a freeze even though the network is fine.
        while filled < out.len() {
            let cursor = start + filled as u64;

            // Serve from an earlier fetched block when it already covers this position.
            if let Some(copied) = self.copy_cached_block(vfs_path, cursor, &mut out[filled..]) {
                filled += copied;
                continue;
            }

            let remaining = out.len() - filled;
            let spans = plan_fetch_spans(cursor, remaining as u64, file_size);
            let Some(&(span_start, span_len)) = spans.first() else {
                break;
            };

            // Extend a *continuing* sequential walk one step further than this read needs.
            //
            // A range request costs a fixed ~300 ms plus ~0.1 ms/KiB, so the marginal byte is
            // nearly free while every fresh request pays that fixed cost again. Fetching the next
            // few windows in the same request turns a start-to-finish walk into roughly one paid
            // round trip per group of windows instead of one per window.
            //
            // Gated on `sequential`, because the saving only exists if the prefetched bytes are
            // actually consumed. A file manager reading 4-16 KiB of each of 625 photos would
            // otherwise transfer megabytes per file that nothing ever reads.
            let sequential = self.is_sequential(vfs_path, span_start);
            let ahead_windows = if sequential { READAHEAD_WINDOWS } else { 0 };
            let readahead_end = span_start
                .saturating_add(span_len)
                .saturating_add(ahead_windows * RANGED_WINDOW_SIZE);
            let extend = readahead_end.min(if file_size > 0 {
                file_size
            } else {
                readahead_end
            });
            let extend = extend
                .saturating_sub(span_start)
                .min(MAX_RANGE_REQUEST_SIZE)
                .max(span_len);

            // The caller holds a download slot for the whole read (see [`Self::read`]), so a burst
            // of reads cannot occupy every FUSE thread and starve `getattr`/`readdir`/`lookup`.
            // This function never acquires one: the gate is not reentrant.
            let Some(bytes) = self.fetch_span(vfs_path, span_start, extend as u32).await else {
                // Nothing was fetched. Answering with a short/empty read here would look like
                // end-of-file and get cached as such by the kernel, so a transient network error
                // would permanently truncate the file. Fail the request instead; the reader
                // retries and hydration has another chance to succeed.
                return Err(EIO);
            };
            if bytes.is_empty() {
                return Err(EIO);
            }

            tracing::debug!(
                vfs_path,
                span_start,
                span_len,
                got = bytes.len(),
                "ranged fetch"
            );

            // Judged against `span_len`, not the readahead-extended `extend`: a response that
            // covers the requested span is complete even though it stops short of the extra
            // readahead bytes, which merely means the server/path had nothing more to give.
            self.record_readahead(vfs_path, span_start, span_len, bytes.len() as u64);

            let short = (bytes.len() as u64) < span_len;

            let skip = (cursor - span_start) as usize;
            if skip >= bytes.len() {
                // Unreachable while spans are window-aligned, but a loop that stops making
                // progress must fail the read rather than spin.
                return Err(EIO);
            }
            let n = (bytes.len() - skip).min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&bytes[skip..skip + n]);
            filled += n;

            // Buffer the span for the next read only after serving it, and move it in: the span
            // is usually a megabyte or more, so cloning it would double the cost of every read.
            self.store_block(vfs_path, span_start, bytes);

            // A short span means we reached the end of the remote file.
            if short {
                break;
            }
        }

        out.truncate(filled);
        Ok(out)
    }

    /// Fetches one contiguous span, splitting it across a few parallel requests.
    ///
    /// Returns `None` when any part failed, and also when a non-final part came back short: a
    /// partially assembled span looks like a contiguous one to the caller, so a short middle part
    /// would silently shift every following byte left and then be cached under the span's offset.
    /// Only the final part may be short, which is how end-of-file inside the span is reported.
    async fn fetch_span(&self, vfs_path: &str, start: u64, len: u32) -> Option<Vec<u8>> {
        if (len as u64) < PARALLEL_SPLIT_MIN {
            return self.fetch_range_shared(vfs_path, start, len).await;
        }

        let part_len = ((len as u64) / MAX_PARALLEL_RANGES as u64).max(1);
        let mut parts: Vec<(u64, u32)> = Vec::with_capacity(MAX_PARALLEL_RANGES);
        let mut offset = start;
        let mut left = u64::from(len);
        while left > 0 {
            let take = left.min(part_len) as u32;
            parts.push((offset, take));
            offset += u64::from(take);
            left -= u64::from(take);
        }

        let total = parts.iter().map(|(_, l)| u64::from(*l)).sum::<u64>();
        let bytes = match parts.as_slice() {
            [(o0, l0)] => self.fetch_range_shared(vfs_path, *o0, *l0).await,
            [(o0, l0), (o1, l1)] => {
                let (a, b) = tokio::join!(
                    self.fetch_range_shared(vfs_path, *o0, *l0),
                    self.fetch_range_shared(vfs_path, *o1, *l1)
                );
                Some(
                    [
                        Self::checked_part(a, *l0, false)?,
                        Self::checked_part(b, *l1, true)?,
                    ]
                    .concat(),
                )
            }
            [(o0, l0), (o1, l1), (o2, l2)] => {
                let (a, b, c) = tokio::join!(
                    self.fetch_range_shared(vfs_path, *o0, *l0),
                    self.fetch_range_shared(vfs_path, *o1, *l1),
                    self.fetch_range_shared(vfs_path, *o2, *l2)
                );
                Some(
                    [
                        Self::checked_part(a, *l0, false)?,
                        Self::checked_part(b, *l1, false)?,
                        Self::checked_part(c, *l2, true)?,
                    ]
                    .concat(),
                )
            }
            [(o0, l0), (o1, l1), (o2, l2), (o3, l3)] => {
                let (a, b, c, d) = tokio::join!(
                    self.fetch_range_shared(vfs_path, *o0, *l0),
                    self.fetch_range_shared(vfs_path, *o1, *l1),
                    self.fetch_range_shared(vfs_path, *o2, *l2),
                    self.fetch_range_shared(vfs_path, *o3, *l3)
                );
                Some(
                    [
                        Self::checked_part(a, *l0, false)?,
                        Self::checked_part(b, *l1, false)?,
                        Self::checked_part(c, *l2, false)?,
                        Self::checked_part(d, *l3, true)?,
                    ]
                    .concat(),
                )
            }
            _ => return self.fetch_range_shared(vfs_path, start, len).await,
        };

        let bytes = bytes?;
        if (bytes.len() as u64) < total {
            tracing::debug!(
                vfs_path,
                start,
                len,
                got = bytes.len(),
                "parallel span fetch reached end of file"
            );
        }
        Some(bytes)
    }

    /// Accepts one part of a split span only when it can be concatenated without shifting bytes.
    ///
    /// `final_part` is allowed to come back short, which is how a span that runs past the end of
    /// the remote file is reported.
    fn checked_part(part: Option<Vec<u8>>, requested: u32, final_part: bool) -> Option<Vec<u8>> {
        let bytes = part?;
        let got = bytes.len() as u64;
        let requested = u64::from(requested);
        if got == requested || (final_part && got < requested) {
            return Some(bytes);
        }
        tracing::warn!(
            requested,
            got,
            final_part,
            "range part does not match its request; failing the span instead of shifting bytes"
        );
        None
    }

    /// Fetches a range, joining an identical request that is already in flight.
    ///
    /// Two readers of the same cold file always reach a window boundary at slightly different
    /// moments, and the kernel issues overlapping reads, so the same window would otherwise be
    /// downloaded once per reader. The first reader publishes its request here; the rest await the
    /// same bytes. Returns `None` when the fetch failed, exactly like [`Self::fetch_range`].
    async fn fetch_range_shared(&self, vfs_path: &str, start: u64, len: u32) -> Option<Vec<u8>> {
        let key: InflightKey = (vfs_path.to_string(), start, len);
        let (cell, joined) = {
            let Ok(mut flights) = self.inflight_ranges.lock() else {
                return self.fetch_range(vfs_path, start, len).await;
            };
            match flights.get(&key) {
                Some(existing) => (existing.clone(), true),
                None => {
                    let cell: InflightRange = Arc::new(tokio::sync::OnceCell::new());
                    flights.insert(key.clone(), cell.clone());
                    (cell, false)
                }
            }
        };

        let value = cell
            .get_or_init(|| async { self.fetch_range(vfs_path, start, len).await })
            .await
            .clone();

        if joined {
            tracing::debug!(
                vfs_path,
                start,
                len,
                got = value.as_ref().map_or(0, Vec::len),
                "joined in-flight range fetch"
            );
        }

        // Only the flight that is still registered may clear the key: a later request for the same
        // window may already have installed a fresh one after this one completed.
        if let Ok(mut flights) = self.inflight_ranges.lock()
            && let Some(registered) = flights.get(&key)
            && Arc::ptr_eq(registered, &cell)
        {
            flights.remove(&key);
        }

        value
    }

    /// Calls the hydrator for `len` bytes at `start`, returning `None` when it fails.
    async fn fetch_range(&self, vfs_path: &str, start: u64, len: u32) -> Option<Vec<u8>> {
        match self.hydrator.hydrate_range(vfs_path, start, len).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::warn!(vfs_path, start, len, error = %err, "ranged hydration failed");
                None
            }
        }
    }

    /// Whether an access at `cursor` continues the walk readahead previously prefetched for.
    ///
    /// True when `cursor` lies inside the range that was prefetched last time, which is what makes
    /// prefetching worthwhile: those bytes are about to be consumed. A jump backwards, or past the
    /// prefetched end, is a different access pattern and gets no readahead.
    fn is_sequential(&self, vfs_path: &str, cursor: u64) -> bool {
        let Ok(cursors) = self.readahead_cursor.read() else {
            return false;
        };
        let Some(&(requested_end, prefetched_end)) = cursors.get(vfs_path) else {
            return false;
        };
        // `<=` on the upper bound matters: when the previous fetch covered exactly one window
        // (no readahead yet), the next sequential access begins precisely at `prefetched_end`.
        // Excluding that boundary would mean readahead never engages at all.
        cursor >= requested_end.saturating_sub(RANGED_WINDOW_SIZE) && cursor <= prefetched_end
    }

    /// Records how far the last fetch for `vfs_path` reached.
    ///
    /// Stores both the end of what the caller needed and the end of what was actually fetched, so
    /// the next access can tell "still consuming the prefetch" from "started somewhere new".
    fn record_readahead(&self, vfs_path: &str, span_start: u64, span_len: u64, got: u64) {
        let Ok(mut cursors) = self.readahead_cursor.write() else {
            return;
        };
        cursors.insert(
            vfs_path.to_string(),
            (
                span_start.saturating_add(span_len),
                span_start.saturating_add(got),
            ),
        );
    }

    /// Returns the buffered block that already covers `cursor`, as `(block_start, bytes)`.
    /// Copies the buffered block covering `cursor` into `out`, returning how many bytes it wrote.
    ///
    /// Copies out of the guarded buffer instead of cloning it: a buffered block is a whole fetch
    /// span, so a 4 KiB read served from one must not clone the rest of it.
    fn copy_cached_block(&self, vfs_path: &str, cursor: u64, out: &mut [u8]) -> Option<usize> {
        let blocks = self.partial_blocks.read().ok()?;
        let entries = blocks.get(vfs_path)?;
        // Newest first: the most recent fetch for a position is the one with the freshest bytes.
        for (start, bytes) in entries.iter().rev() {
            let Some(skip) = cursor
                .checked_sub(*start)
                .filter(|skip| *skip < bytes.len() as u64)
            else {
                continue;
            };
            let skip = skip as usize;
            let n = (bytes.len() - skip).min(out.len());
            out[..n].copy_from_slice(&bytes[skip..skip + n]);
            return Some(n);
        }
        None
    }

    /// Buffers a fetched block so subsequent reads of the same window stay local.
    fn store_block(&self, vfs_path: &str, start: u64, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        let Ok(mut blocks) = self.partial_blocks.write() else {
            return;
        };
        if blocks.len() >= MAX_CACHED_FILES && !blocks.contains_key(vfs_path) {
            // No `forget`/`unlink` hook reaches this cache, so a file that disappeared would
            // otherwise keep its buffers for the lifetime of the mount.
            if let Some(stale) = blocks.keys().next().cloned() {
                blocks.remove(&stale);
            }
        }
        let entry = blocks.entry(vfs_path.to_string()).or_default();
        // A block that fully covers an older one makes it unreachable for every future read, so
        // drop it now instead of letting overlapping copies accumulate.
        let end = start.saturating_add(bytes.len() as u64);
        entry.retain(|(existing_start, existing)| {
            let existing_end = existing_start.saturating_add(existing.len() as u64);
            !(*existing_start >= start && existing_end <= end)
        });
        entry.push((start, bytes));
        // Evict oldest-first. Evicting by offset instead would throw away the readahead that was
        // just fetched and keep the stale windows at the start of the file.
        while entry.len() > MAX_CACHED_BLOCKS_PER_FILE {
            entry.remove(0);
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
    // `secs` comes straight from server metadata and a numeric string can carry
    // `u64::MAX`; adding that to the epoch would panic inside getattr.
    SystemTime::UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(secs))
        .unwrap_or(SystemTime::UNIX_EPOCH)
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
    // No Nextcloud timestamp predates the epoch by much or postdates this
    // range, and bounding the year keeps the day arithmetic below from
    // overflowing for a hostile input such as year `i64::MIN`.
    if !(1900..=9999).contains(&y) {
        return None;
    }
    let y = y - i64::from(m <= 2);
    let era = y / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe as i64 - 719468;
    if days < 0 {
        return None;
    }
    (days as u64)
        .checked_mul(86400)?
        .checked_add(h * 3600 + min * 60 + sec)
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

    /// Regression: a node left pointing at a cache file that was deleted must hydrate again.
    ///
    /// The tree persists hydration as a path, so the file can be gone after the cache directory was
    /// cleared or deleted. Answering `EIO` forever made the file unreadable until a restart.
    #[tokio::test]
    async fn missing_cache_file_hydrates_again() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();

        let dir = std::env::temp_dir().join(format!("nexia-fuse-gone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cached = dir.join("photo.jpg");
        std::fs::write(&cached, &payload).unwrap();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/photo.jpg").with_size(payload.len() as u64),
        ));
        tree.hydrate_path("/photo.jpg", cached.clone()).unwrap();
        std::fs::remove_file(&cached).unwrap();

        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "photo.jpg").unwrap();

        assert_eq!(driver.read(attr.ino, 0, 256).await.unwrap(), payload[..256]);

        let _ = std::fs::remove_dir(&dir);
    }

    /// Regression: a split span whose middle part comes back short must fail, not shift bytes.
    ///
    /// The parts are concatenated in order, so a short middle part moved every following byte to a
    /// lower offset while still looking like a full-length span — the read returned wrong bytes and
    /// cached them under the span's offset. A short *final* part is different: that is how a span
    /// that runs past the end of the remote file is reported.
    #[test]
    fn a_short_middle_part_fails_the_span() {
        let short = Some(vec![0u8; 9]);
        assert!(FuseDriver::checked_part(short, 10, false).is_none());

        let short = Some(vec![0u8; 9]);
        assert!(FuseDriver::checked_part(short, 10, true).is_some());

        // A part that returns *more* than it was asked for would also shift the bytes after it.
        let long = Some(vec![0u8; 11]);
        assert!(FuseDriver::checked_part(long, 10, true).is_none());

        assert!(FuseDriver::checked_part(None, 10, true).is_none());
    }

    /// Regression: the block cache must keep the most recently fetched spans.
    ///
    /// It sorted by offset and drained from the front, so it kept the windows at the start of the
    /// file and threw away the readahead that had just been fetched — the opposite of what the
    /// readahead policy in [`READAHEAD_WINDOWS`] relies on.
    #[test]
    fn the_block_cache_keeps_the_newest_spans() {
        let driver = FuseDriver::new(Arc::new(RwLock::new(VirtualTree::new("root"))));

        for start in 0..20u64 {
            driver.store_block("/a.bin", start * 10, vec![start as u8; 10]);
        }

        let blocks = driver.partial_blocks.read().unwrap();
        let starts: Vec<u64> = blocks["/a.bin"].iter().map(|(start, _)| *start).collect();
        assert_eq!(starts.len(), MAX_CACHED_BLOCKS_PER_FILE);
        assert_eq!(
            starts.first(),
            Some(&40),
            "the oldest spans are evicted first"
        );
        assert_eq!(starts.last(), Some(&190), "the newest span survives");
    }

    /// A span that fully covers an older one makes the older one unreachable, so it is dropped.
    #[test]
    fn a_covering_span_replaces_the_spans_it_covers() {
        let driver = FuseDriver::new(Arc::new(RwLock::new(VirtualTree::new("root"))));

        driver.store_block("/a.bin", 0, vec![1u8; 10]);
        driver.store_block("/a.bin", 10, vec![2u8; 10]);
        driver.store_block("/a.bin", 0, vec![3u8; 20]);

        let blocks = driver.partial_blocks.read().unwrap();
        let entries: Vec<(u64, Vec<u8>)> = blocks["/a.bin"].clone();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, 0);
        assert_eq!(entries[0].1, vec![3u8; 20]);

        // The newest copy is the one served.
        let mut out = vec![0u8; 4];
        assert_eq!(driver.copy_cached_block("/a.bin", 12, &mut out), Some(4));
        assert_eq!(out, vec![3u8; 4]);

        let mut out = vec![0u8; 64];
        assert_eq!(driver.copy_cached_block("/a.bin", 16, &mut out), Some(4));
        assert_eq!(driver.copy_cached_block("/a.bin", 20, &mut out), None);
    }

    /// A desynchronised parallel fetch must surface as `EIO` rather than as wrong bytes.
    ///
    /// The hydrator returns one byte less than asked for on the second part of a four-way split,
    /// which is what a server that clamps a `Range` header does.
    #[tokio::test]
    async fn a_desynchronised_parallel_fetch_returns_eio() {
        // Four windows of payload, and a read wide enough to be split into four parts of one
        // window each.
        let size = 8 * RANGED_WINDOW_SIZE;
        let read_len = (2 * RANGED_WINDOW_SIZE) as u32;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();

        struct ShortMiddlePartHydrator {
            payload: Vec<u8>,
            short_at: u64,
        }

        impl AsyncHydrator for ShortMiddlePartHydrator {
            fn hydrate(
                &self,
                path: &str,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
            {
                let path = path.to_string();
                Box::pin(async move { Err(VfsError::NotFound(path)) })
            }

            fn hydrate_range(
                &self,
                _path: &str,
                offset: u64,
                len: u32,
            ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
            {
                let start = (offset as usize).min(self.payload.len());
                let mut end = (start + len as usize).min(self.payload.len());
                if offset == self.short_at {
                    end = end.saturating_sub(1);
                }
                let slice = self.payload[start..end].to_vec();
                Box::pin(async move { Ok(slice) })
            }
        }

        // The second of the four parts a two-window read is split into.
        let hydrator = Arc::new(ShortMiddlePartHydrator {
            payload: payload.clone(),
            short_at: RANGED_WINDOW_SIZE,
        });

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/big.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "big.jpg").unwrap();

        assert_eq!(driver.read(attr.ino, 0, read_len).await, Err(EIO));
    }

    /// Records what `hydrate_range` was asked for and serves from an in-memory buffer.
    struct RangeHydrator {
        payload: Vec<u8>,
        full_fails: bool,
        range_calls: Arc<RwLock<Vec<(u64, u32)>>>,
        /// Number of times the *whole file* was requested, which is what must not happen for a
        /// large file.
        full_calls: Arc<RwLock<Vec<String>>>,
    }

    impl RangeHydrator {
        fn new(payload: Vec<u8>, full_fails: bool) -> Self {
            Self {
                payload,
                full_fails,
                range_calls: Arc::new(RwLock::new(Vec::new())),
                full_calls: Arc::new(RwLock::new(Vec::new())),
            }
        }
    }

    impl AsyncHydrator for RangeHydrator {
        fn hydrate(
            &self,
            canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            self.full_calls
                .write()
                .unwrap()
                .push(canonical_path.to_string());
            let fail = self.full_fails;
            let path = canonical_path.to_string();
            let payload = self.payload.clone();
            Box::pin(async move {
                if fail {
                    Err(VfsError::NotFound(path))
                } else {
                    Ok(payload)
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

    /// A hydrator whose ranged reads take long enough for two readers to overlap.
    struct SlowRangeHydrator {
        payload: Vec<u8>,
        calls: Arc<RwLock<Vec<(u64, u32)>>>,
        delay: std::time::Duration,
    }

    impl AsyncHydrator for SlowRangeHydrator {
        fn hydrate(
            &self,
            canonical_path: &str,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            let path = canonical_path.to_string();
            Box::pin(async move { Err(VfsError::NotFound(path)) })
        }

        fn hydrate_range(
            &self,
            _canonical_path: &str,
            offset: u64,
            size: u32,
        ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>
        {
            self.calls.write().unwrap().push((offset, size));
            let start = (offset as usize).min(self.payload.len());
            let end = (start + size as usize).min(self.payload.len());
            let slice = self.payload[start..end].to_vec();
            let delay = self.delay;
            Box::pin(async move {
                // Deliberately a blocking sleep: the crate does not enable `tokio/time`, and this
                // test needs a guaranteed overlap window rather than a scheduler-dependent one.
                std::thread::sleep(delay);
                Ok(slice)
            })
        }
    }

    /// Two concurrent readers of one cold file must share a single range request.
    ///
    /// A cold sequential read is a pipeline: the kernel issues overlapping reads, so two FUSE
    /// threads reach the same window at almost the same moment. Each used to fetch that window
    /// independently — measured on a real mount, every window after the first was downloaded
    /// twice — which doubled both the transferred bytes and the time to open a cold file.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_readers_share_one_range_request() {
        let size = 4 * RANGED_WINDOW_SIZE;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let calls = Arc::new(RwLock::new(Vec::new()));
        let hydrator = Arc::new(SlowRangeHydrator {
            payload: payload.clone(),
            calls: calls.clone(),
            delay: std::time::Duration::from_millis(200),
        });

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/big.jpg").with_size(size),
        ));
        let driver = Arc::new(FuseDriver::with_hydrator(
            Arc::new(RwLock::new(tree)),
            hydrator,
        ));
        let attr = driver.lookup(FUSE_ROOT_INODE, "big.jpg").unwrap();

        let first = {
            let driver = driver.clone();
            tokio::spawn(async move { driver.read(attr.ino, 0, 4096).await })
        };
        let second = {
            let driver = driver.clone();
            tokio::spawn(async move { driver.read(attr.ino, 0, 4096).await })
        };

        let got_first = first.await.unwrap().unwrap();
        let got_second = second.await.unwrap().unwrap();
        assert_eq!(got_first, payload[..4096]);
        assert_eq!(got_second, payload[..4096]);

        let recorded = calls.read().unwrap().clone();
        assert_eq!(
            recorded.len(),
            1,
            "an identical in-flight range must be shared, not fetched twice: {recorded:?}"
        );
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
            full_calls: Arc::new(RwLock::new(Vec::new())),
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

    /// A file above the threshold must never be fetched whole to answer a small read.
    ///
    /// This is the freeze that made a file manager hang on a photo directory: every touch of a
    /// large file pulled the entire payload, holding one FUSE thread for the whole transfer, so a
    /// handful of concurrent reads exhausted the thread pool and no request could complete. The
    /// read must stay ranged. One byte over the limit is enough to switch behaviour.
    #[tokio::test]
    async fn large_placeholder_is_read_in_ranges_not_wholesale() {
        let size = FULL_HYDRATION_MAX_SIZE + 1;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let full_calls = hydrator.full_calls.clone();
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/big.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "big.jpg").unwrap();

        let got = driver.read(attr.ino, 0, 4096).await.unwrap();
        assert_eq!(got, payload[..4096]);

        assert!(
            full_calls.read().unwrap().is_empty(),
            "a {size}-byte file was hydrated wholesale to serve 4 KiB"
        );
        let ranges = range_calls.read().unwrap().clone();
        assert_eq!(
            ranges.len(),
            1,
            "expected exactly one ranged fetch: {ranges:?}"
        );
        assert_eq!(ranges[0].0, 0);
        // The fetched size may exceed one window because readahead extends the request, but it must
        // stay small relative to the file: the point of ranging is that touching a file costs a
        // bounded amount, not its full size.
        assert!(
            u64::from(ranges[0].1) <= RANGED_WINDOW_SIZE * (1 + READAHEAD_WINDOWS),
            "ranged fetch was larger than one window plus readahead: {:?}",
            ranges[0]
        );
    }

    /// Touching a very large file must cost a bounded amount, not its full size.
    ///
    /// The neighbouring test uses a file only one byte over the threshold, where fetching the whole
    /// thing is genuinely optimal and readahead correctly does so. This one uses a file far larger
    /// than a window plus readahead, which is the case that used to hang a file manager.
    #[tokio::test]
    async fn touching_a_huge_placeholder_fetches_a_bounded_amount() {
        let size = 32 * 1024 * 1024;
        let hydrator = Arc::new(RangeHydrator::new(vec![7u8; size as usize], false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/huge.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "huge.jpg").unwrap();

        let got = driver.read(attr.ino, 0, 4096).await.unwrap();
        assert_eq!(got.len(), 4096);

        let calls = range_calls.read().unwrap().clone();
        assert_eq!(calls.len(), 1, "a 4 KiB read cost {} requests", calls.len());
        let fetched = u64::from(calls[0].1);
        assert!(
            fetched <= RANGED_WINDOW_SIZE * (1 + READAHEAD_WINDOWS),
            "a 4 KiB read fetched {fetched} bytes of a {size}-byte file"
        );
    }

    /// Paging through a file must not re-download bytes it already has.
    ///
    /// This is the property that matters for sequential readers, and readahead serves it better
    /// than one-request-per-window: it must reduce the number of round trips, never increase the
    /// number of times the same byte range is fetched. The assertion is therefore on *coverage* —
    /// every byte of the file fetched exactly once, with no overlapping ranges — rather than on a
    /// fixed request count.
    #[tokio::test]
    async fn ranged_windows_do_not_overlap_when_paging_forward() {
        let size = 8 * RANGED_WINDOW_SIZE;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/big.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "big.jpg").unwrap();

        // Walk the file one read at a time, each read just past the previous window boundary.
        let step = RANGED_WINDOW_SIZE as i64;
        for i in 0..size as i64 / step {
            let offset = i * step;
            let got = driver.read(attr.ino, offset, 4096).await.unwrap();
            assert_eq!(got, payload[offset as usize..offset as usize + 4096]);
        }

        let calls = range_calls.read().unwrap().clone();
        assert!(!calls.is_empty(), "paging forward issued no fetch at all");

        // Readahead must be a net win: far fewer round trips than one per window even though a
        // large span is itself fetched as up to [`MAX_PARALLEL_RANGES`] parallel requests.
        let windows = (size / RANGED_WINDOW_SIZE) as usize;
        assert!(
            calls.len() <= windows * MAX_PARALLEL_RANGES,
            "readahead made paging worse: {} fetches for {windows} windows: {calls:?}",
            calls.len()
        );

        // Ranges must be disjoint and must together cover every byte exactly once, so a forward
        // walk never pays twice for the same data.
        let mut spans: Vec<(u64, u64)> = calls.iter().map(|(o, l)| (*o, u64::from(*l))).collect();
        spans.sort_unstable();
        let mut covered = 0u64;
        for (offset, len) in spans {
            assert!(
                offset <= covered,
                "overlapping or out-of-order fetch at {offset} (already covered {covered})"
            );
            assert!(
                offset + len <= size,
                "fetch {offset}+{len} reads past the {size}-byte file"
            );
            covered = covered.max(offset + len);
        }
        assert_eq!(covered, size, "a forward walk did not fetch the whole file");
    }

    /// A single large read must stay a bounded number of requests, not one per window.
    ///
    /// This is the freeze that a live session exhibited: an image loader asked for 8.8-9.2 MiB in
    /// one `read`, which is 18 windows of [`RANGED_WINDOW_SIZE`]. Fetching window by window put 18
    /// round trips in series on one FUSE thread, and at ~0.5 s per hop that is nine seconds of
    /// apparently-hung file manager for a read the caller had already committed to. The windows are
    /// merged into contiguous spans, and a span large enough to be worth it is fetched as a few
    /// parallel requests rather than one long one.
    #[tokio::test]
    async fn one_large_read_issues_a_single_range_request() {
        let size = 18 * RANGED_WINDOW_SIZE;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/huge.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "huge.jpg").unwrap();

        let got = driver.read(attr.ino, 0, size as u32).await.unwrap();
        assert_eq!(got, payload, "the merged read returned the wrong bytes");

        let calls = range_calls.read().unwrap().clone();
        assert!(
            calls.len() <= MAX_PARALLEL_RANGES,
            "one {size}-byte read was split into {} round trips: {calls:?}",
            calls.len()
        );
        assert!(
            calls.len() > 1,
            "a {size}-byte read must be spread over parallel requests, not fetched as one: {calls:?}"
        );

        // Whatever the split, the parts must be contiguous and cover the file exactly once.
        let mut spans: Vec<(u64, u64)> = calls.iter().map(|(o, l)| (*o, u64::from(*l))).collect();
        spans.sort_unstable();
        let mut covered = 0u64;
        for (offset, len) in spans {
            assert_eq!(
                offset, covered,
                "span starts at {offset}, expected {covered}"
            );
            covered += len;
        }
        assert_eq!(covered, size, "the parts did not cover the whole file");
    }

    /// A read in the middle of a file must not be rounded up past the end of the file.
    ///
    /// Span ends are rounded up to a window boundary so sequential readers find the rest of the
    /// window already buffered, but that rounding must never make a request extend beyond the
    /// remote file: some servers answer a request for bytes past the end with `416` instead of a
    /// short body.
    #[tokio::test]
    async fn trailing_span_is_clamped_to_the_file_size() {
        let size = 2 * RANGED_WINDOW_SIZE + 4096;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/tail.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "tail.jpg").unwrap();

        let start = size - 4096;
        let got = driver.read(attr.ino, start as i64, 4096).await.unwrap();
        assert_eq!(got, payload[start as usize..]);

        for (offset, len) in range_calls.read().unwrap().iter().copied() {
            assert!(
                offset + u64::from(len) <= size,
                "range request {offset}+{len} reads past the {size}-byte file"
            );
        }
    }

    /// A first touch of a large file must not prefetch, because nothing proves the bytes are wanted.
    ///
    /// A file manager building thumbnails for a photo directory reads a few KiB of each file and
    /// moves on. Prefetching four windows on every first touch would transfer megabytes per photo
    /// that nothing ever reads, which is worse than the latency it saves.
    #[tokio::test]
    async fn first_touch_of_a_large_file_does_not_readahead() {
        let size = 32 * 1024 * 1024;
        let hydrator = Arc::new(RangeHydrator::new(vec![1u8; size as usize], false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/photo.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "photo.jpg").unwrap();

        driver.read(attr.ino, 0, 4096).await.unwrap();

        let calls = range_calls.read().unwrap().clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            u64::from(calls[0].1),
            RANGED_WINDOW_SIZE,
            "a first touch prefetched beyond its own window: {calls:?}"
        );
    }

    /// Once a reader is demonstrably sequential, the following accesses must prefetch.
    #[tokio::test]
    async fn a_sequential_walk_reads_ahead_after_the_first_window() {
        let size = 16 * RANGED_WINDOW_SIZE;
        let payload: Vec<u8> = (0..=255u8).cycle().take(size as usize).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let range_calls = hydrator.range_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/walk.jpg").with_size(size),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "walk.jpg").unwrap();

        // Walk forward in 4 KiB steps, as a reader streaming the file would.
        let mut offset = 0i64;
        while (offset as u64) < size {
            let got = driver.read(attr.ino, offset, 4096).await.unwrap();
            assert_eq!(got, payload[offset as usize..offset as usize + 4096]);
            offset += 4096;
        }

        let calls = range_calls.read().unwrap().clone();
        let windows = (size / RANGED_WINDOW_SIZE) as usize;
        assert!(
            calls.len() < windows,
            "a sequential walk made {} requests for {windows} windows, so readahead never engaged",
            calls.len()
        );
        assert!(
            calls
                .iter()
                .any(|(_, len)| u64::from(*len) > RANGED_WINDOW_SIZE),
            "no request ever exceeded one window: {calls:?}"
        );
    }

    /// The download gate must serialise down to its limit and never deadlock or leak slots.
    #[test]
    fn download_gate_serialises_to_its_limit() {
        let gate = Arc::new(DownloadGate::new(3));
        let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let live = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        std::thread::scope(|scope| {
            for _ in 0..24 {
                let gate = Arc::clone(&gate);
                let peak = Arc::clone(&peak);
                let live = Arc::clone(&live);
                scope.spawn(move || {
                    let _slot = gate.acquire();
                    let now = live.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    peak.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    live.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                });
            }
        });

        assert_eq!(
            peak.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "more downloads ran at once than the gate allows"
        );
        assert_eq!(gate.in_flight(), 0, "a slot was leaked by a dropped guard");
    }

    /// The gate must be able to answer "would this read park?" without blocking, because that
    /// answer is what stops a file-manager sweep from occupying the whole shared thread pool.
    #[test]
    fn download_gate_reports_saturation_without_blocking() {
        let gate = Arc::new(DownloadGate::new(2));
        assert!(!gate.is_saturated(), "a fresh gate must not look saturated");
        assert_eq!(gate.available(), 2);

        let first = gate.acquire().expect("first slot");
        assert!(!gate.is_saturated());
        assert_eq!(gate.available(), 1);

        let second = gate.acquire().expect("second slot");
        assert!(
            gate.is_saturated(),
            "a full gate must report saturation so callers decline instead of parking"
        );
        assert_eq!(gate.available(), 0);

        drop(first);
        assert!(
            !gate.is_saturated(),
            "releasing a slot must clear saturation"
        );
        drop(second);
        assert_eq!(gate.available(), 2);
    }

    /// An unknown file size must still be range-read, never fetched wholesale.
    #[tokio::test]
    async fn zero_size_placeholder_uses_ranges() {
        let payload = vec![3u8; 4 * RANGED_WINDOW_SIZE as usize];
        let hydrator = Arc::new(RangeHydrator::new(payload, false));
        let full_calls = hydrator.full_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(NodeMetadata::file("/unknown.bin")));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "unknown.bin").unwrap();

        let _ = driver.read(attr.ino, 0, 4096).await;
        assert!(
            full_calls.read().unwrap().is_empty(),
            "a file with unknown size was hydrated wholesale"
        );
    }

    /// A small placeholder still hydrates in one piece: up to one ranged window it costs no more
    /// than the ranged fetch it replaces, and leaves the whole file readable.
    #[tokio::test]
    async fn small_placeholder_is_hydrated_wholesale() {
        let payload = vec![7u8; 64 * 1024];
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), true));
        let full_calls = hydrator.full_calls.clone();

        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/small.png").with_size(payload.len() as u64),
        ));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "small.png").unwrap();

        let got = driver.read(attr.ino, 0, 4096).await.unwrap();
        assert_eq!(got, payload[..4096]);
        assert_eq!(
            full_calls.read().unwrap().len(),
            1,
            "a small file should hydrate in a single round trip"
        );
    }

    /// A file whose reported size is 0 (unknown) must avoid a wholesale fetch.
    ///
    /// Size is unknown precisely when the server did not tell us, so it could be anything; the
    /// safe reading is "too large", since guessing wrong means downloading something unbounded.
    #[tokio::test]
    async fn unknown_size_placeholder_avoids_wholesale_fetch() {
        let payload: Vec<u8> = (0..=255u8).cycle().take(256 * 1024).collect();
        let hydrator = Arc::new(RangeHydrator::new(payload.clone(), false));
        let full_calls = hydrator.full_calls.clone();

        let mut tree = VirtualTree::new("root");
        // No `.with_size(...)`: size stays 0, i.e. unknown.
        tree.insert(VirtualNode::new_file(NodeMetadata::file("/nosize.bin")));
        let driver = FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator);
        let attr = driver.lookup(FUSE_ROOT_INODE, "nosize.bin").unwrap();

        assert!(driver.read(attr.ino, 0, 1024).await.is_ok());
        assert!(
            full_calls.read().unwrap().is_empty(),
            "an unknown-size file must not be fetched wholesale"
        );
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
