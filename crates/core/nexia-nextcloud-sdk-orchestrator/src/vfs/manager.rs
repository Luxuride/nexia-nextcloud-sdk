//! In-memory virtual tree orchestrator with persistent disk caching and hydration tracking.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::error::{OrchestratorError, OrchestratorResult};
use nexia_nextcloud_sdk_vfs::{
    DirectorySizeSummary, NodeMetadata, VirtualEntrySummary, VirtualNode, VirtualTree,
};

/// High-level virtual file system manager maintaining metadata trees and file hydration state.
#[derive(Debug)]
pub struct VfsManager {
    tree: VirtualTree,
    cache_dir: PathBuf,
}

impl VfsManager {
    /// Creates a new `VfsManager` storing cached files and `tree.json` in `cache_dir`.
    pub fn new(cache_dir: impl Into<PathBuf>, root_name: &str) -> Self {
        let cache_dir = cache_dir.into();
        let _ = fs::create_dir_all(&cache_dir);

        let tree_file = cache_dir.join("tree.json");
        let initial_tree = if tree_file.exists() {
            fs::read_to_string(&tree_file)
                .ok()
                .and_then(|data| serde_json::from_str(&data).ok())
                .unwrap_or_else(|| VirtualTree::new(root_name))
        } else {
            VirtualTree::new(root_name)
        };

        Self {
            tree: initial_tree,
            cache_dir,
        }
    }

    /// Access the base cache directory path.
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Returns an immutable reference to the virtual tree.
    #[must_use]
    pub fn tree(&self) -> &VirtualTree {
        &self.tree
    }

    /// Returns a mutable reference to the virtual tree.
    pub fn tree_mut(&mut self) -> &mut VirtualTree {
        &mut self.tree
    }

    /// Replaces the virtual tree in memory.
    pub fn set_tree(&mut self, tree: VirtualTree) {
        self.tree = tree;
    }

    /// Access the root node display name.
    #[must_use]
    pub fn root_name(&self) -> String {
        self.tree
            .get("/")
            .map(|n| n.metadata.name.clone())
            .unwrap_or_else(|| "Nextcloud".to_string())
    }

    /// Returns an entry summary for the given virtual path if present in the tree.
    #[must_use]
    pub fn entry(&self, path: &str) -> Option<VirtualEntrySummary> {
        self.tree.entry_summary(path)
    }

    /// Lists direct children of the specified directory path.
    #[must_use]
    pub fn list_children(&self, path: &str) -> Vec<VirtualEntrySummary> {
        self.tree.children_summaries(path)
    }

    /// Returns `true` if the directory exists and has been populated with its children from remote WebDAV.
    #[must_use]
    pub fn is_directory_populated(&self, path: &str) -> bool {
        self.tree.is_directory_populated(path)
    }

    /// Returns a list of directory paths that have not yet been populated with children from remote WebDAV.
    #[must_use]
    pub fn unpopulated_directories(&self) -> Vec<String> {
        self.tree.unpopulated_directories()
    }

    /// Computes recursive directory size and item count statistics.
    #[must_use]
    pub fn directory_size_summary(&self, path: &str) -> DirectorySizeSummary {
        self.tree.directory_size_summary(path)
    }

    /// Maps a remote-relative path to its location inside the cache directory.
    ///
    /// Rejects anything that could escape `cache_dir`: an absolute path or a
    /// path with a `.`/`..`/empty segment. Every caller resolves the local path
    /// through here so the check cannot be forgotten on one of them.
    fn cached_file_path(&self, relative_path: &str) -> OrchestratorResult<PathBuf> {
        let clean = relative_path.trim_start_matches('/');
        let rejected = clean.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
        });
        if rejected || clean.is_empty() {
            return Err(OrchestratorError::InvalidPath {
                path: relative_path.to_string(),
                reason: "path must be a non-empty relative path inside the cache directory"
                    .to_string(),
            });
        }
        Ok(self.cache_dir.join(clean))
    }

    /// Hydrates a virtual file locally by writing its content to the cache directory.
    ///
    /// `relative_path` comes from a remote listing or a caller, so it is checked
    /// before it touches the filesystem: a path that climbs out of the cache
    /// directory would otherwise let a server-supplied name write anywhere the
    /// process can.
    pub fn hydrate_local(
        &mut self,
        relative_path: &str,
        content: &[u8],
    ) -> OrchestratorResult<PathBuf> {
        let local_file_path = self.cached_file_path(relative_path)?;

        if let Some(parent) = local_file_path.parent() {
            fs::create_dir_all(parent).map_err(|err| OrchestratorError::Io {
                path: parent.to_path_buf(),
                context: "creating directory for hydrated file".to_string(),
                source: err,
            })?;
        }

        fs::write(&local_file_path, content).map_err(|err| OrchestratorError::Io {
            path: local_file_path.clone(),
            context: "writing hydrated file content".to_string(),
            source: err,
        })?;

        // Ensure the node exists in the tree as a file before marking hydrated
        if !self.tree.contains(relative_path) {
            let mut meta = NodeMetadata::file(relative_path);
            meta.size = content.len() as u64;
            let node = VirtualNode::new_file(meta);
            self.tree.insert(node);
        }

        self.tree
            .hydrate_path(relative_path, local_file_path.clone())
            .map_err(|e| OrchestratorError::Vfs {
                operation: "hydrate_path".to_string(),
                source: e,
            })?;

        self.persist_tree()?;
        debug!(path = %relative_path, "hydrated file locally");
        Ok(local_file_path)
    }

    /// Dehydrates a locally cached file, deleting its local bytes and reverting to a placeholder.
    pub fn dehydrate_local(&mut self, relative_path: &str) -> OrchestratorResult<()> {
        let local_file_path = self.cached_file_path(relative_path)?;

        if local_file_path.exists() {
            let _ = fs::remove_file(&local_file_path);
        }

        self.tree
            .dehydrate(relative_path)
            .map_err(|e| OrchestratorError::Vfs {
                operation: "dehydrate".to_string(),
                source: e,
            })?;

        self.persist_tree()?;
        debug!(path = %relative_path, "dehydrated file locally");
        Ok(())
    }

    /// Calculates the number of hydrated files and their aggregated bytes on disk.
    #[must_use]
    pub fn calculate_hydrated_storage(&self) -> (usize, u64) {
        let mut count = 0;
        let mut total_bytes = 0;
        for path in self.tree.paths() {
            if let Some(node) = self.tree.get(path)
                && !node.hydration.is_placeholder()
            {
                count += 1;
                match &node.hydration {
                    nexia_nextcloud_sdk_vfs::HydrationState::Hydrated(
                        nexia_nextcloud_sdk_vfs::HydratedContent::Path(p),
                    ) => {
                        // Only bytes actually present in the cache file count. Falling back to
                        // `node.metadata.size` would report the server-side size of a file that is
                        // not on disk, which is exactly the number users compare with `du`.
                        if let Ok(meta) = fs::metadata(p) {
                            total_bytes += meta.len();
                        }
                    }
                    nexia_nextcloud_sdk_vfs::HydrationState::Hydrated(
                        nexia_nextcloud_sdk_vfs::HydratedContent::Bytes(b),
                    ) => {
                        total_bytes += b.len() as u64;
                    }
                    _ => {}
                }
            }
        }
        (count, total_bytes)
    }

    /// Dehydrates all locally hydrated files in this VFS, deleting cache files from disk and returning freed bytes.
    pub fn clear_all_hydrated(&mut self) -> (usize, u64) {
        let paths: Vec<String> = self
            .tree
            .paths()
            .filter(|p| {
                self.tree
                    .get(p)
                    .map(|n| !n.hydration.is_placeholder())
                    .unwrap_or(false)
            })
            .map(|s| s.to_string())
            .collect();

        let mut count = 0;
        let mut freed = 0;
        for path in paths {
            if let Some(node) = self.tree.get(&path)
                && let nexia_nextcloud_sdk_vfs::HydrationState::Hydrated(
                    nexia_nextcloud_sdk_vfs::HydratedContent::Path(p),
                ) = &node.hydration
            {
                if let Ok(meta) = fs::metadata(p) {
                    freed += meta.len();
                }
                let _ = fs::remove_file(p);
            }
            if self.tree.dehydrate(&path).is_ok() {
                count += 1;
            }
        }
        let _ = self.persist_tree();
        (count, freed)
    }

    /// Adds a new directory node to the virtual tree.
    pub fn create_directory(&mut self, path: &str) -> OrchestratorResult<()> {
        let meta = NodeMetadata::directory(path);
        let node = VirtualNode::new_directory(meta);
        self.tree.insert(node);
        self.persist_tree()
    }

    /// Adds a new file node with metadata to the virtual tree.
    pub fn create_file(&mut self, metadata: NodeMetadata) -> OrchestratorResult<()> {
        let node = VirtualNode::new_file(metadata);
        self.tree.insert(node);
        self.persist_tree()
    }

    /// Renames a virtual file or directory entry.
    pub fn rename(&mut self, old_path: &str, new_name: &str) -> OrchestratorResult<()> {
        self.tree
            .rename(old_path, new_name)
            .map_err(|e| OrchestratorError::Vfs {
                operation: "rename".to_string(),
                source: e,
            })?;

        self.persist_tree()
    }

    /// Moves a virtual file or directory entry to a new parent location.
    pub fn move_entry(
        &mut self,
        source_path: &str,
        destination_path: &str,
    ) -> OrchestratorResult<()> {
        self.tree
            .move_node(source_path, destination_path)
            .map_err(|e| OrchestratorError::Vfs {
                operation: "move_entry".to_string(),
                source: e,
            })?;

        self.persist_tree()
    }

    /// Removes a file or directory entry from the virtual tree.
    pub fn remove(&mut self, path: &str) -> OrchestratorResult<()> {
        if self.tree.remove(path).is_none() {
            return Err(OrchestratorError::InvalidPath {
                path: path.to_string(),
                reason: "node not found in virtual tree".to_string(),
            });
        }

        self.persist_tree()
    }

    /// Serializes the virtual tree to `tree.json` in the cache directory.
    ///
    /// Hydrated payloads are spilled into `<cache_dir>/content` first, so the tree records a cache
    /// path rather than the file content: the persisted tree stays metadata-sized, and a restart
    /// reuses the downloaded bytes instead of fetching them again.
    pub fn persist_tree(&mut self) -> OrchestratorResult<()> {
        let tree_file = self.cache_dir.join("tree.json");
        let content_dir = self.cache_dir.join("content");

        if let Err(err) = self.tree.spill_hydrated_bytes(&content_dir) {
            // Persisting metadata is still worthwhile: an unspilled payload only means the file is
            // fetched again after a restart.
            debug!(
                content_dir = %content_dir.display(),
                error = %err,
                "could not spill hydrated payloads before persisting the tree"
            );
        }

        let json = serde_json::to_string_pretty(&self.tree)?;
        // Write then rename, so an interrupted save cannot leave a half-written tree that fails to
        // parse and silently drops every known node on the next start.
        let temp_file = self.cache_dir.join("tree.json.tmp");
        fs::write(&temp_file, json).map_err(|err| OrchestratorError::Io {
            path: temp_file.clone(),
            context: "writing virtual tree to disk".to_string(),
            source: err,
        })?;
        fs::rename(&temp_file, &tree_file).map_err(|err| OrchestratorError::Io {
            path: tree_file,
            context: "persisting virtual tree to disk".to_string(),
            source: err,
        })
    }
}
