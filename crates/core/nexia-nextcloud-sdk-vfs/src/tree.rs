//! In-memory hierarchical virtual file system tree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dir_size::DirectorySizeSummary;
use crate::entry::VirtualEntrySummary;
use crate::error::VfsError;
use crate::hydration::{HydratedContent, HydrationState};
use crate::metadata::NodeMetadata;
use crate::node::VirtualNode;
use crate::path::{normalize_path, parent_path};

/// An in-memory hierarchical tree of virtual file system nodes.
///
/// Nodes are indexed by their canonical paths within the mount (e.g. `"/"`,
/// `"/Photos"`, `"/Photos/vacation.jpg"`). Directories maintain links to their
/// children, and file contents can transition between metadata-only placeholders
/// and hydrated payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualTree {
    nodes: HashMap<String, VirtualNode>,
}

impl VirtualTree {
    /// Creates a new virtual tree with a root directory named `root_name`.
    pub fn new(root_name: impl Into<String>) -> Self {
        let root_meta = NodeMetadata::directory("/").with_name(root_name);
        let root_node = VirtualNode::new_directory(root_meta);

        let mut nodes = HashMap::new();
        nodes.insert("/".to_string(), root_node);

        Self { nodes }
    }

    /// Inserts a node into the virtual tree.
    ///
    /// If the path has a parent directory currently present in the tree, this node
    /// is registered as a child of that parent. If the node is a directory, any
    /// previously inserted children belonging to it are attached.
    pub fn insert(&mut self, mut node: VirtualNode) {
        let canonical = match normalize_path(&node.metadata.path) {
            Ok(p) => p,
            Err(_) => node.metadata.path.clone(),
        };
        tracing::debug!(path = %canonical, is_dir = node.metadata.is_directory, "Inserting node into VirtualTree");
        node.metadata.path = canonical.clone();

        if node.metadata.is_directory {
            let mut children = node.children.unwrap_or_default();
            // Preserve existing children if replacing a directory node
            if let Some(existing_children) = self
                .nodes
                .get(&canonical)
                .and_then(|existing| existing.children.as_ref())
            {
                children.extend(existing_children.iter().cloned());
            }
            // Adopt existing nodes whose parent is this directory
            for existing_path in self.nodes.keys() {
                if parent_path(existing_path).as_deref() == Some(&canonical) {
                    children.insert(existing_path.clone());
                }
            }
            node.children = Some(children);
        }

        if let Some(existing) = self.nodes.get(&canonical) {
            if node.metadata.is_directory {
                let etag_changed = existing.metadata.etag.is_some()
                    && node.metadata.etag.is_some()
                    && existing.metadata.etag != node.metadata.etag;
                let has_empty_children_with_content =
                    node.metadata.size > 0 && node.children.as_ref().is_none_or(|c| c.is_empty());

                if etag_changed || has_empty_children_with_content {
                    node.is_populated = false;
                } else {
                    node.is_populated = existing.is_populated;
                }
            }
            if node.metadata.size == 0 && existing.metadata.size > 0 {
                node.metadata.size = existing.metadata.size;
            }
            if !node.metadata.is_directory {
                // Only carry the hydrated payload over when the remote object is unchanged.
                // A changed ETag (or a changed byte length) means the cached bytes belong to an
                // older revision of the file, and keeping them made every later read serve stale
                // content — and, for `HydratedContent::Path` entries, content read from a cache
                // file that no longer matched the node.
                let content_changed = existing.metadata.etag.is_some()
                    && node.metadata.etag.is_some()
                    && existing.metadata.etag != node.metadata.etag;
                let size_changed = existing.metadata.size > 0
                    && node.metadata.size > 0
                    && existing.metadata.size != node.metadata.size;

                if existing.is_hydrated() && !content_changed && !size_changed {
                    node.hydration = existing.hydration.clone();
                }
                if !content_changed && !size_changed {
                    node.thumbnails = existing.thumbnails.clone();
                }
            }
        }

        // Link with parent directory if present
        if let Some(children) = parent_path(&canonical)
            .and_then(|parent| self.nodes.get_mut(&parent))
            .and_then(|parent_node| parent_node.children.as_mut())
        {
            children.insert(canonical.clone());
        }

        self.nodes.insert(canonical, node);
    }

    /// Looks up a reference to a node by its path.
    pub fn get(&self, path: &str) -> Option<&VirtualNode> {
        let normalized = normalize_path(path).ok()?;
        self.nodes.get(&normalized)
    }

    /// Looks up a mutable reference to a node by its path.
    pub fn get_mut(&mut self, path: &str) -> Option<&mut VirtualNode> {
        let normalized = normalize_path(path).ok()?;
        self.nodes.get_mut(&normalized)
    }

    /// Returns a list of direct children nodes for a given directory path.
    ///
    /// Returns an empty vector if the directory does not exist or has no children.
    pub fn children(&self, dir_path: &str) -> Vec<&VirtualNode> {
        let normalized = match normalize_path(dir_path) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        let Some(node) = self.nodes.get(&normalized) else {
            return Vec::new();
        };

        if !node.is_directory() {
            return Vec::new();
        }

        let Some(child_paths) = &node.children else {
            return Vec::new();
        };

        child_paths
            .iter()
            .filter_map(|p| self.nodes.get(p))
            .collect()
    }

    /// Computes the recursive aggregate size and item counts for a directory or file.
    ///
    /// If `path` refers to a file, the summary contains its size, `file_count: 1`, and `directory_count: 0`.
    /// If `path` refers to a directory, all descendant files and subfolders are traversed recursively.
    /// If `path` does not exist in the tree, returns [`DirectorySizeSummary::empty()`].
    pub fn directory_size_summary(&self, path: &str) -> DirectorySizeSummary {
        let Ok(normalized) = normalize_path(path) else {
            return DirectorySizeSummary::empty();
        };

        let Some(root_node) = self.get(&normalized) else {
            return DirectorySizeSummary::empty();
        };

        if !root_node.is_directory() {
            let is_hydrated = root_node.is_hydrated();
            let is_unsynced = root_node.metadata.etag.is_none();
            return DirectorySizeSummary {
                total_size: root_node.metadata.size,
                file_count: 1,
                directory_count: 0,
                has_unhydrated: !is_hydrated,
                has_unsynced: is_unsynced,
                direct_size: root_node.metadata.size,
                direct_file_count: 1,
                direct_directory_count: 0,
            };
        }

        let mut summary = DirectorySizeSummary::empty();
        let mut queue = Vec::new();

        if let Some(children) = &root_node.children {
            for child_path in children {
                if let Some(child_node) = self.get(child_path) {
                    if child_node.is_directory() {
                        summary.direct_directory_count += 1;
                    } else {
                        summary.direct_file_count += 1;
                        summary.direct_size += child_node.metadata.size;
                    }
                }
                queue.push(child_path.clone());
            }
        }

        while let Some(child_path) = queue.pop() {
            if let Some(node) = self.get(&child_path) {
                if node.is_directory() {
                    summary.directory_count += 1;
                    if let Some(sub_children) = &node.children {
                        for sub_child_path in sub_children {
                            queue.push(sub_child_path.clone());
                        }
                    }
                } else {
                    summary.file_count += 1;
                    summary.total_size = summary.total_size.saturating_add(node.metadata.size);
                    if !node.is_hydrated() {
                        summary.has_unhydrated = true;
                    }
                    if node.metadata.etag.is_none() {
                        summary.has_unsynced = true;
                    }
                }
            }
        }

        if summary.total_size == 0 && root_node.metadata.size > 0 {
            summary.total_size = root_node.metadata.size;
        }

        summary
    }

    /// Returns the recursive total size in bytes of all descendant files within `dir_path`.
    pub fn directory_size(&self, dir_path: &str) -> u64 {
        self.directory_size_summary(dir_path).total_size
    }

    /// Returns entry summaries for all direct children of `dir_path`, computing recursive folder sizes.
    pub fn children_summaries(&self, dir_path: &str) -> Vec<VirtualEntrySummary> {
        let Ok(normalized) = normalize_path(dir_path) else {
            return Vec::new();
        };

        self.children(&normalized)
            .into_iter()
            .map(|node| VirtualEntrySummary::from_node_with_tree(node, self))
            .collect()
    }

    /// Returns an entry summary for `path`, computing recursive folder size if it is a directory.
    pub fn entry_summary(&self, path: &str) -> Option<VirtualEntrySummary> {
        let normalized = normalize_path(path).ok()?;
        self.get(&normalized)
            .map(|node| VirtualEntrySummary::from_node_with_tree(node, self))
    }

    /// Checks whether a directory exists in the virtual tree and has had its children populated from remote WebDAV.
    pub fn is_directory_populated(&self, path: &str) -> bool {
        let Ok(normalized) = normalize_path(path) else {
            return false;
        };
        self.nodes
            .get(&normalized)
            .map(|n| {
                n.is_directory()
                    && n.is_populated
                    && !(n.metadata.size > 0 && n.children.as_ref().is_none_or(|c| c.is_empty()))
            })
            .unwrap_or(false)
    }

    /// Sets whether a directory node has been populated from remote WebDAV.
    pub fn set_directory_populated(&mut self, path: &str, populated: bool) {
        if let Ok(normalized) = normalize_path(path)
            && let Some(node) = self.nodes.get_mut(&normalized)
            && node.is_directory()
        {
            node.is_populated = populated;
        }
    }

    /// Returns a sorted list of all directory paths in the tree that have not yet had their children populated.
    pub fn unpopulated_directories(&self) -> Vec<String> {
        let mut dirs: Vec<String> = self
            .nodes
            .values()
            .filter(|n| {
                n.is_directory()
                    && (!n.is_populated
                        || (n.metadata.size > 0
                            && n.children.as_ref().is_none_or(|c| c.is_empty())))
            })
            .map(|n| n.metadata.path.clone())
            .collect();
        dirs.sort();
        dirs
    }

    /// Hydrates a file node with in-memory byte content.
    ///
    /// # Errors
    ///
    /// - [`VfsError::NotFound`] if the path does not exist in the tree.
    /// - [`VfsError::NotAFile`] if the path refers to a directory.
    /// - [`VfsError::InvalidPath`] if the path format is malformed.
    pub fn hydrate(&mut self, path: &str, content: Vec<u8>) -> Result<(), VfsError> {
        let normalized = normalize_path(path)?;
        tracing::info!(path = %normalized, bytes = content.len(), "Hydrating virtual file node");
        let node = self.nodes.get_mut(&normalized).ok_or_else(|| {
            tracing::warn!(path = %normalized, "Failed to hydrate node: not found");
            VfsError::NotFound(normalized.clone())
        })?;

        if node.is_directory() {
            tracing::warn!(path = %normalized, "Failed to hydrate node: not a file");
            return Err(VfsError::NotAFile(normalized));
        }

        node.hydration = HydrationState::Hydrated(HydratedContent::Bytes(content));
        Ok(())
    }

    /// Reads the buffered bytes of `path`, reading through the local cache file when the node is
    /// hydrated via [`HydratedContent::Path`].
    ///
    /// Returns `Ok(None)` when the node carries no readable payload at all (a placeholder). This is
    /// the accessor callers must use instead of [`HydrationState::bytes`], which silently reports
    /// "no data" for path-backed nodes.
    ///
    /// # Errors
    ///
    /// - [`VfsError::NotFound`] if the path does not exist in the tree.
    /// - [`VfsError::InvalidPath`] if the path format is malformed.
    pub fn read_hydrated_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, VfsError> {
        let normalized = normalize_path(path)?;
        let node = self
            .nodes
            .get(&normalized)
            .ok_or_else(|| VfsError::NotFound(normalized.clone()))?;
        match node.hydration() {
            HydrationState::Hydrated(HydratedContent::Bytes(bytes)) => Ok(Some(bytes.clone())),
            HydrationState::Hydrated(HydratedContent::Path(cached)) => {
                match std::fs::read(cached) {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(err) => {
                        tracing::warn!(
                            path = %normalized,
                            cached = %cached.display(),
                            error = %err,
                            "hydrated cache file is unreadable"
                        );
                        Ok(None)
                    }
                }
            }
            HydrationState::Placeholder => Ok(None),
        }
    }

    /// Returns `true` if any node holds its payload in memory instead of a cache file.
    ///
    /// Used to detect a tree written by an older build, where payloads were serialized inline and a
    /// single account's `tree.json` reached hundreds of megabytes.
    #[must_use]
    pub fn has_in_memory_payloads(&self) -> bool {
        self.nodes.values().any(|node| {
            matches!(
                node.hydration,
                HydrationState::Hydrated(HydratedContent::Bytes(_))
            )
        })
    }

    /// Writes every in-memory payload below `content_root` and points the node at that file.
    ///
    /// Call this before persisting a tree: an in-memory payload is never serialized (see
    /// [`HydrationState`]), so without spilling, a downloaded file would come back as a
    /// placeholder and be fetched again. Each payload is written at its own path below
    /// `content_root` — the layout the FUSE driver reads through — so the tree only ever stores a
    /// path, not the bytes.
    ///
    /// Returns the number of payloads written.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when a directory or a file cannot be created or written.
    /// A node whose payload could not be written keeps its in-memory payload.
    pub fn spill_hydrated_bytes(&mut self, content_root: &Path) -> std::io::Result<usize> {
        let mut spilled = 0;

        for node in self.nodes.values_mut() {
            let payload = match std::mem::replace(node.hydration_mut(), HydrationState::Placeholder)
            {
                HydrationState::Hydrated(HydratedContent::Bytes(bytes)) if !bytes.is_empty() => {
                    bytes
                }
                other => {
                    *node.hydration_mut() = other;
                    continue;
                }
            };

            let relative = node.metadata.path.trim_start_matches('/').to_string();
            let escapes = relative.is_empty() || relative.split('/').any(|s| s == "..");
            if escapes {
                *node.hydration_mut() = HydrationState::Hydrated(HydratedContent::Bytes(payload));
                continue;
            }

            let target: PathBuf = content_root.join(&relative);
            let written = target
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&target, &payload));

            match written {
                Ok(()) => {
                    *node.hydration_mut() = HydrationState::Hydrated(HydratedContent::Path(target));
                    spilled += 1;
                }
                Err(err) => {
                    *node.hydration_mut() =
                        HydrationState::Hydrated(HydratedContent::Bytes(payload));
                    return Err(err);
                }
            }
        }

        Ok(spilled)
    }

    /// Merges freshly discovered remote metadata into this tree without discarding local state.
    ///
    /// This is the entry point to use when refreshing a tree that a live FUSE mount is serving:
    /// it differs from [`Self::populate_from_resources`] in that it **never** removes existing
    /// nodes, so a partial refresh (a single directory PROPFIND) cannot wipe out sibling subtrees,
    /// and it preserves in-flight hydrations and downloaded thumbnails via [`Self::insert`].
    ///
    /// Returns the number of resources applied.
    pub fn merge_from_resources(
        &mut self,
        base_dav_prefix: &str,
        resources: &[crate::resource::DavResourceSummary],
    ) -> usize {
        merge_resources_into(self, base_dav_prefix, resources)
    }

    /// Copies hydration state and thumbnails from `source` onto every file node whose remote
    /// revision is unchanged.
    ///
    /// Used right after replacing a live tree with a snapshot: the snapshot is generally older
    /// than the running mount, and without this every file that was hydrated in the meantime
    /// silently reverts to a placeholder and has to be downloaded again.
    ///
    /// Returns the number of nodes that kept their hydrated payload.
    pub fn adopt_hydration_from(&mut self, source: &Self) -> usize {
        let mut adopted = 0;
        for path in source.paths().map(str::to_string).collect::<Vec<_>>() {
            let Some(src) = source.get(&path) else {
                continue;
            };
            if src.is_directory() || !src.is_hydrated() {
                continue;
            }
            let Some(dst) = self.nodes.get_mut(&path) else {
                continue;
            };
            if dst.is_directory() {
                continue;
            }
            let revision_changed = src.metadata.etag.is_some()
                && dst.metadata.etag.is_some()
                && src.metadata.etag != dst.metadata.etag;
            let size_changed = src.metadata.size > 0
                && dst.metadata.size > 0
                && src.metadata.size != dst.metadata.size;
            if revision_changed || size_changed {
                continue;
            }
            dst.hydration = src.hydration.clone();
            if dst.thumbnails.is_empty() {
                dst.thumbnails = src.thumbnails.clone();
            }
            adopted += 1;
        }
        adopted
    }

    /// Hydrates a file node with a local filesystem cache path.
    ///
    /// # Errors
    ///
    /// - [`VfsError::NotFound`] if the path does not exist in the tree.
    /// - [`VfsError::NotAFile`] if the path refers to a directory.
    /// - [`VfsError::InvalidPath`] if the path format is malformed.
    pub fn hydrate_path(
        &mut self,
        path: &str,
        cached_path: std::path::PathBuf,
    ) -> Result<(), VfsError> {
        let normalized = normalize_path(path)?;
        let node = self
            .nodes
            .get_mut(&normalized)
            .ok_or_else(|| VfsError::NotFound(normalized.clone()))?;

        if node.is_directory() {
            return Err(VfsError::NotAFile(normalized));
        }

        node.hydration = HydrationState::Hydrated(HydratedContent::Path(cached_path));
        Ok(())
    }

    /// Dehydrates a file node back to placeholder state to reclaim memory.
    ///
    /// # Errors
    ///
    /// - [`VfsError::NotFound`] if the path does not exist in the tree.
    /// - [`VfsError::NotAFile`] if the path refers to a directory.
    /// - [`VfsError::InvalidPath`] if the path format is malformed.
    pub fn dehydrate(&mut self, path: &str) -> Result<(), VfsError> {
        let normalized = normalize_path(path)?;
        tracing::info!(path = %normalized, "Dehydrating virtual file node");
        let node = self.nodes.get_mut(&normalized).ok_or_else(|| {
            tracing::warn!(path = %normalized, "Failed to dehydrate node: not found");
            VfsError::NotFound(normalized.clone())
        })?;

        if node.is_directory() {
            tracing::warn!(path = %normalized, "Failed to dehydrate node: not a file");
            return Err(VfsError::NotAFile(normalized));
        }

        node.hydration = HydrationState::Placeholder;
        Ok(())
    }

    /// Stores rendered thumbnail bytes for a node at the given resolution.
    ///
    /// # Errors
    ///
    /// - [`VfsError::NotFound`] if the path does not exist in the tree.
    /// - [`VfsError::InvalidPath`] if the path format is malformed.
    pub fn set_thumbnail(
        &mut self,
        path: &str,
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    ) -> Result<(), VfsError> {
        let normalized = normalize_path(path)?;
        let node = self
            .nodes
            .get_mut(&normalized)
            .ok_or(VfsError::NotFound(normalized))?;

        node.thumbnails.insert(width, height, bytes);
        Ok(())
    }

    /// Retrieves cached thumbnail bytes for the specified dimensions, if present.
    pub fn get_thumbnail(&self, path: &str, width: u32, height: u32) -> Option<&[u8]> {
        let normalized = normalize_path(path).ok()?;
        self.nodes
            .get(&normalized)
            .and_then(|n| n.thumbnails.get(width, height))
    }

    /// Removes a node and any of its nested descendants from the virtual tree.
    ///
    /// Returns the removed [`VirtualNode`], or `None` if the node was not found.
    pub fn remove(&mut self, path: &str) -> Option<VirtualNode> {
        let normalized = normalize_path(path).ok()?;

        // Unlink from parent directory
        if let Some(parent_node) =
            parent_path(&normalized).and_then(|parent| self.nodes.get_mut(&parent))
        {
            parent_node.remove_child(&normalized);
        }

        // Recursively remove nested children if this node is a directory
        let prefix = if normalized == "/" {
            "/".to_string()
        } else {
            format!("{normalized}/")
        };

        let descendant_keys: Vec<String> = self
            .nodes
            .keys()
            .filter(|k| k.starts_with(&prefix) && *k != &normalized)
            .cloned()
            .collect();

        for child in descendant_keys {
            self.nodes.remove(&child);
        }

        self.nodes.remove(&normalized)
    }

    /// Returns the total number of nodes stored in the virtual tree.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the tree contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns `true` if a node exists at the specified path.
    pub fn contains(&self, path: &str) -> bool {
        self.get(path).is_some()
    }

    /// Returns an iterator over all canonical paths currently in the tree.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.nodes.keys().map(String::as_str)
    }

    /// Renames a file or directory node in the virtual tree, recursively updating all
    /// descendant node paths and directory children sets.
    ///
    /// # Errors
    ///
    /// - [`VfsError::CannotModifyRoot`] if attempting to rename `"/"`.
    /// - [`VfsError::NotFound`] if `old_path` does not exist in the tree.
    /// - [`VfsError::InvalidPath`] if `new_name` is empty or contains path separators (`/`).
    /// - [`VfsError::TargetAlreadyExists`] if a node already exists at the computed target path.
    pub fn rename(&mut self, old_path: &str, new_name: &str) -> Result<String, VfsError> {
        let canonical_old = normalize_path(old_path)?;
        if canonical_old == "/" {
            return Err(VfsError::CannotModifyRoot);
        }

        if !self.nodes.contains_key(&canonical_old) {
            return Err(VfsError::NotFound(canonical_old));
        }

        let trimmed_new = new_name.trim().trim_matches('/');
        if trimmed_new.is_empty() || trimmed_new.contains('/') {
            return Err(VfsError::InvalidPath(format!(
                "invalid new name '{new_name}'"
            )));
        }

        let parent = parent_path(&canonical_old).unwrap_or_else(|| "/".to_string());
        let canonical_new = if parent == "/" {
            format!("/{trimmed_new}")
        } else {
            format!("{parent}/{trimmed_new}")
        };

        if canonical_old == canonical_new {
            return Ok(canonical_new);
        }

        if self.nodes.contains_key(&canonical_new) {
            return Err(VfsError::TargetAlreadyExists(canonical_new));
        }

        // Unlink old path from parent and link new path
        if let Some(parent_node) = self.nodes.get_mut(&parent) {
            parent_node.remove_child(&canonical_old);
            parent_node.add_child(&canonical_new);
        }

        let mut node = self
            .nodes
            .remove(&canonical_old)
            .ok_or_else(|| VfsError::NotFound(canonical_old.clone()))?;
        node.metadata.path = canonical_new.clone();
        node.metadata.name = trimmed_new.to_string();

        if node.is_directory() {
            let old_prefix = format!("{canonical_old}/");
            let new_prefix = format!("{canonical_new}/");

            let descendant_keys: Vec<String> = self
                .nodes
                .keys()
                .filter(|k| k.starts_with(&old_prefix))
                .cloned()
                .collect();

            let mut updated_descendants = Vec::new();
            for old_desc_path in descendant_keys {
                if let Some(mut desc_node) = self.nodes.remove(&old_desc_path) {
                    let suffix = &old_desc_path[old_prefix.len()..];
                    let new_desc_path = format!("{new_prefix}{suffix}");
                    desc_node.metadata.path = new_desc_path.clone();

                    if desc_node.is_directory()
                        && let Some(children) = desc_node.children.as_mut()
                    {
                        let updated_children: std::collections::BTreeSet<String> = children
                            .iter()
                            .map(|c| {
                                if c.starts_with(&old_prefix) {
                                    format!("{new_prefix}{}", &c[old_prefix.len()..])
                                } else {
                                    c.clone()
                                }
                            })
                            .collect();
                        *children = updated_children;
                    }
                    updated_descendants.push((new_desc_path, desc_node));
                }
            }

            // Update children of node itself
            if let Some(children) = node.children.as_mut() {
                let updated_children: std::collections::BTreeSet<String> = children
                    .iter()
                    .map(|c| {
                        if c.starts_with(&old_prefix) {
                            format!("{new_prefix}{}", &c[old_prefix.len()..])
                        } else {
                            c.clone()
                        }
                    })
                    .collect();
                *children = updated_children;
            }

            for (desc_path, desc_node) in updated_descendants {
                self.nodes.insert(desc_path, desc_node);
            }
        }

        self.nodes.insert(canonical_new.clone(), node);
        Ok(canonical_new)
    }

    /// Moves a file or directory node in the virtual tree to `target_dir_path`, recursively
    /// updating all descendant node paths and directory linkages.
    ///
    /// # Errors
    ///
    /// - [`VfsError::CannotModifyRoot`] if attempting to move `"/"`.
    /// - [`VfsError::NotFound`] if `source_path` or `target_dir_path` does not exist.
    /// - [`VfsError::NotADirectory`] if `target_dir_path` is a file node.
    /// - [`VfsError::CannotMoveIntoSelf`] if attempting to move a directory into itself or one of its descendants.
    /// - [`VfsError::TargetAlreadyExists`] if a node with the same name already exists in `target_dir_path`.
    pub fn move_node(
        &mut self,
        source_path: &str,
        target_dir_path: &str,
    ) -> Result<String, VfsError> {
        let canonical_src = normalize_path(source_path)?;
        if canonical_src == "/" {
            return Err(VfsError::CannotModifyRoot);
        }

        let canonical_target = normalize_path(target_dir_path)?;

        if !self.nodes.contains_key(&canonical_src) {
            return Err(VfsError::NotFound(canonical_src));
        }

        let target_is_dir = self
            .nodes
            .get(&canonical_target)
            .map(|n| n.is_directory())
            .ok_or_else(|| VfsError::NotFound(canonical_target.clone()))?;
        if !target_is_dir {
            return Err(VfsError::NotADirectory(canonical_target));
        }

        if canonical_target == canonical_src
            || canonical_target.starts_with(&format!("{canonical_src}/"))
        {
            return Err(VfsError::CannotMoveIntoSelf(canonical_target));
        }

        let item_name = canonical_src
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        let canonical_dest = if canonical_target == "/" {
            format!("/{item_name}")
        } else {
            format!("{canonical_target}/{item_name}")
        };

        if canonical_src == canonical_dest {
            return Ok(canonical_dest);
        }

        if self.nodes.contains_key(&canonical_dest) {
            return Err(VfsError::TargetAlreadyExists(canonical_dest));
        }

        // Unlink from old parent
        let old_parent = parent_path(&canonical_src).unwrap_or_else(|| "/".to_string());
        if let Some(parent_node) = self.nodes.get_mut(&old_parent) {
            parent_node.remove_child(&canonical_src);
        }

        // Link into new parent
        if let Some(target_node) = self.nodes.get_mut(&canonical_target) {
            target_node.add_child(&canonical_dest);
        }

        let mut node = self
            .nodes
            .remove(&canonical_src)
            .ok_or_else(|| VfsError::NotFound(canonical_src.clone()))?;
        node.metadata.path = canonical_dest.clone();

        if node.is_directory() {
            let old_prefix = format!("{canonical_src}/");
            let new_prefix = format!("{canonical_dest}/");

            let descendant_keys: Vec<String> = self
                .nodes
                .keys()
                .filter(|k| k.starts_with(&old_prefix))
                .cloned()
                .collect();

            let mut updated_descendants = Vec::new();
            for old_desc_path in descendant_keys {
                if let Some(mut desc_node) = self.nodes.remove(&old_desc_path) {
                    let suffix = &old_desc_path[old_prefix.len()..];
                    let new_desc_path = format!("{new_prefix}{suffix}");
                    desc_node.metadata.path = new_desc_path.clone();

                    if desc_node.is_directory()
                        && let Some(children) = desc_node.children.as_mut()
                    {
                        let updated_children: std::collections::BTreeSet<String> = children
                            .iter()
                            .map(|c| {
                                if c.starts_with(&old_prefix) {
                                    format!("{new_prefix}{}", &c[old_prefix.len()..])
                                } else {
                                    c.clone()
                                }
                            })
                            .collect();
                        *children = updated_children;
                    }
                    updated_descendants.push((new_desc_path, desc_node));
                }
            }

            // Update children of node itself
            if let Some(children) = node.children.as_mut() {
                let updated_children: std::collections::BTreeSet<String> = children
                    .iter()
                    .map(|c| {
                        if c.starts_with(&old_prefix) {
                            format!("{new_prefix}{}", &c[old_prefix.len()..])
                        } else {
                            c.clone()
                        }
                    })
                    .collect();
                *children = updated_children;
            }

            for (desc_path, desc_node) in updated_descendants {
                self.nodes.insert(desc_path, desc_node);
            }
        }

        self.nodes.insert(canonical_dest.clone(), node);
        Ok(canonical_dest)
    }

    /// Populates and synchronizes the virtual tree from a list of discovered remote WebDAV resources,
    /// ensuring all intermediate parent directories exist automatically.
    ///
    /// Children of the listed directory that the server no longer reports are pruned. Use
    /// [`Self::merge_from_resources`] instead when the refresh is partial or when a live mount is
    /// reading from this tree.
    ///
    /// Returns the number of resources inserted.
    pub fn populate_from_resources(
        &mut self,
        base_dav_prefix: &str,
        resources: &[crate::resource::DavResourceSummary],
    ) -> usize {
        self.apply_resources(base_dav_prefix, resources, true)
    }

    fn apply_resources(
        &mut self,
        base_dav_prefix: &str,
        resources: &[crate::resource::DavResourceSummary],
        prune: bool,
    ) -> usize {
        let mut inserted = 0;

        let mut target_dir = None;
        if let Some(first) = resources.first() {
            let clean_path = extract_clean_dav_path(&first.href, base_dav_prefix);
            if first.is_collection {
                target_dir = Some(clean_path);
            }
        }

        for res in resources {
            let clean_path = extract_clean_dav_path(&res.href, base_dav_prefix);

            if clean_path == "/" {
                continue;
            }

            if let Some(parent) = parent_path(&clean_path) {
                self.ensure_directory_chain(&parent);
            }

            let node = if res.is_collection {
                let mut meta = NodeMetadata::directory(&clean_path);
                if let Some(len) = res.content_length {
                    meta = meta.with_size(len);
                }
                if let Some(ref etag) = res.etag {
                    meta = meta.with_etag(etag.clone());
                }
                if let Some(ref lm) = res.last_modified {
                    meta = meta.with_modified_at(lm.clone());
                }
                if let Some(id) = res.file_id {
                    meta = meta.with_file_id(id);
                }
                VirtualNode::new_directory(meta)
            } else {
                let mut meta =
                    NodeMetadata::file(&clean_path).with_size(res.content_length.unwrap_or(0));
                if let Some(ref etag) = res.etag {
                    meta = meta.with_etag(etag.clone());
                }
                if let Some(ref lm) = res.last_modified {
                    meta = meta.with_modified_at(lm.clone());
                }
                if let Some(ref mime) = res.content_type {
                    meta = meta.with_mime_type(mime.clone());
                }
                if let Some(has_prev) = res.has_preview {
                    meta = meta.with_has_preview(has_prev);
                }
                if let Some(id) = res.file_id {
                    meta = meta.with_file_id(id);
                }
                VirtualNode::new_file(meta)
            };

            self.insert(node);
            inserted += 1;
        }

        if let Some(target) = target_dir {
            self.set_directory_populated(&target, true);

            // Prune any children of `target` that were moved or deleted on the remote server
            let returned_children: std::collections::HashSet<String> = resources
                .iter()
                .map(|r| extract_clean_dav_path(&r.href, base_dav_prefix))
                .filter(|p| p != &target)
                .collect();

            if prune {
                let current_children = self.children(&target);
                let stale_children: Vec<String> = current_children
                    .into_iter()
                    .map(|c| c.path().to_string())
                    .filter(|p| !returned_children.contains(p))
                    .collect();

                for stale in stale_children {
                    self.remove(&stale);
                }
            } else {
                let _ = returned_children;
            }
        }

        inserted
    }

    fn ensure_directory_chain(&mut self, path: &str) {
        if path == "/" || self.contains(path) {
            return;
        }
        if let Some(parent) = parent_path(path) {
            self.ensure_directory_chain(&parent);
        }
        self.insert(VirtualNode::new_directory(NodeMetadata::directory(path)));
    }
}

/// Shared resource-application loop for [`VirtualTree::populate_from_resources`] and
/// [`VirtualTree::merge_from_resources`].
fn merge_resources_into(
    tree: &mut VirtualTree,
    base_dav_prefix: &str,
    resources: &[crate::resource::DavResourceSummary],
) -> usize {
    tree.apply_resources(base_dav_prefix, resources, false)
}

fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                let hex_str = [h1, h2];
                if let Ok(hex) = std::str::from_utf8(&hex_str)
                    && let Ok(val) = u8::from_str_radix(hex, 16)
                {
                    bytes.push(val);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).unwrap_or_else(|_| s.to_string())
}

fn extract_clean_dav_path(href: &str, base_dav_prefix: &str) -> String {
    let decoded = percent_decode(href);
    let prefix = base_dav_prefix.trim_end_matches('/');

    let path_after_prefix = if let Some(idx) = decoded.find(prefix) {
        &decoded[idx + prefix.len()..]
    } else if let Some(idx) = decoded.find("/remote.php/dav/files/") {
        let after_files = &decoded[idx + "/remote.php/dav/files/".len()..];
        if let Some(slash) = after_files.find('/') {
            &after_files[slash..]
        } else {
            ""
        }
    } else if let Some(idx) = decoded.find("/remote.php/webdav") {
        &decoded[idx + "/remote.php/webdav".len()..]
    } else if let Some(p) = decoded.strip_prefix(prefix) {
        p
    } else if let Some(idx) = decoded.find("://") {
        let after_scheme = &decoded[idx + 3..];
        if let Some(first_slash) = after_scheme.find('/') {
            &after_scheme[first_slash..]
        } else {
            &decoded
        }
    } else {
        &decoded
    };

    let trimmed = path_after_prefix.trim_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_nested_tree_hierarchies() {
        let mut tree = VirtualTree::new("root");
        assert_eq!(tree.len(), 1);
        assert!(tree.contains("/"));

        let photos = VirtualNode::new_directory(NodeMetadata::directory("/Photos"));
        tree.insert(photos);

        let vacation = VirtualNode::new_file(
            NodeMetadata::file("/Photos/vacation.jpg")
                .with_size(1024)
                .with_has_preview(true),
        );
        tree.insert(vacation);

        assert_eq!(tree.len(), 3);
        assert!(tree.contains("/Photos"));
        assert!(tree.contains("/Photos/vacation.jpg"));

        let root_children = tree.children("/");
        assert_eq!(root_children.len(), 1);
        assert_eq!(root_children[0].path(), "/Photos");

        let photo_children = tree.children("/Photos");
        assert_eq!(photo_children.len(), 1);
        assert_eq!(photo_children[0].path(), "/Photos/vacation.jpg");
    }

    #[test]
    fn listing_directory_children() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Documents",
        )));
        tree.insert(VirtualNode::new_file(NodeMetadata::file(
            "/Documents/doc1.txt",
        )));
        tree.insert(VirtualNode::new_file(NodeMetadata::file(
            "/Documents/doc2.txt",
        )));

        let children = tree.children("/Documents");
        assert_eq!(children.len(), 2);
        let names: Vec<&str> = children.iter().map(|n| n.name()).collect();
        assert_eq!(names, vec!["doc1.txt", "doc2.txt"]);
    }

    #[test]
    fn placeholder_status_size_from_metadata() {
        let mut tree = VirtualTree::new("root");
        let file_meta = NodeMetadata::file("/large_archive.zip")
            .with_size(50_000_000)
            .with_etag("etag-zip-1");
        tree.insert(VirtualNode::new_file(file_meta));

        let node = tree.get("/large_archive.zip").unwrap();
        assert!(node.is_placeholder());
        assert!(!node.is_hydrated());
        assert_eq!(node.size(), 50_000_000);
        assert_eq!(node.hydration.bytes(), None);
    }

    #[test]
    fn hydration_and_dehydration_transitions() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/notes.txt").with_size(12),
        ));

        let node = tree.get("/notes.txt").unwrap();
        assert!(node.is_placeholder());

        // Hydrate
        let payload = b"hello world\n".to_vec();
        tree.hydrate("/notes.txt", payload.clone()).unwrap();

        let hydrated_node = tree.get("/notes.txt").unwrap();
        assert!(hydrated_node.is_hydrated());
        assert!(!hydrated_node.is_placeholder());
        assert_eq!(hydrated_node.hydration.bytes(), Some(payload.as_slice()));

        // Dehydrate
        tree.dehydrate("/notes.txt").unwrap();
        let dehydrated_node = tree.get("/notes.txt").unwrap();
        assert!(dehydrated_node.is_placeholder());
        assert_eq!(dehydrated_node.hydration.bytes(), None);
    }

    #[test]
    fn thumbnail_storage_and_retrieval() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/photo.png").with_has_preview(true),
        ));

        let thumb_64 = vec![1, 2, 3, 4];
        let thumb_256 = vec![5, 6, 7, 8];

        tree.set_thumbnail("/photo.png", 64, 64, thumb_64.clone())
            .unwrap();
        tree.set_thumbnail("/photo.png", 256, 256, thumb_256.clone())
            .unwrap();

        assert_eq!(
            tree.get_thumbnail("/photo.png", 64, 64),
            Some(thumb_64.as_slice())
        );
        assert_eq!(
            tree.get_thumbnail("/photo.png", 256, 256),
            Some(thumb_256.as_slice())
        );
        assert_eq!(tree.get_thumbnail("/photo.png", 512, 512), None);
    }

    #[test]
    fn error_conditions() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Photos",
        )));

        // Access non-existent
        assert_eq!(tree.get("/nonexistent.txt"), None);
        assert!(matches!(
            tree.hydrate("/nonexistent.txt", vec![1]),
            Err(VfsError::NotFound(_))
        ));
        assert!(matches!(
            tree.dehydrate("/nonexistent.txt"),
            Err(VfsError::NotFound(_))
        ));
        assert!(matches!(
            tree.set_thumbnail("/nonexistent.txt", 64, 64, vec![1]),
            Err(VfsError::NotFound(_))
        ));

        // Hydrating a directory
        assert!(matches!(
            tree.hydrate("/Photos", vec![1, 2, 3]),
            Err(VfsError::NotAFile(_))
        ));
        assert!(matches!(
            tree.dehydrate("/Photos"),
            Err(VfsError::NotAFile(_))
        ));
    }

    #[test]
    fn removal_cleans_up_subtree_and_parents() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Photos",
        )));
        tree.insert(VirtualNode::new_file(NodeMetadata::file("/Photos/a.jpg")));
        tree.insert(VirtualNode::new_file(NodeMetadata::file("/Photos/b.jpg")));

        assert_eq!(tree.children("/Photos").len(), 2);

        // Remove single file
        let removed_a = tree.remove("/Photos/a.jpg");
        assert!(removed_a.is_some());
        assert_eq!(tree.children("/Photos").len(), 1);
        assert!(!tree.contains("/Photos/a.jpg"));

        // Remove directory
        let removed_dir = tree.remove("/Photos");
        assert!(removed_dir.is_some());
        assert!(!tree.contains("/Photos"));
        assert!(!tree.contains("/Photos/b.jpg"));
        assert_eq!(tree.children("/").len(), 0);
    }

    #[test]
    fn rename_file_and_directory_subtrees() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Documents",
        )));
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Documents/Projects",
        )));
        tree.insert(VirtualNode::new_file(NodeMetadata::file(
            "/Documents/Projects/spec.md",
        )));

        // Cannot rename root
        assert_eq!(tree.rename("/", "Other"), Err(VfsError::CannotModifyRoot));

        // Rename file
        let new_file = tree
            .rename("/Documents/Projects/spec.md", "specification.md")
            .unwrap();
        assert_eq!(new_file, "/Documents/Projects/specification.md");
        assert!(tree.contains("/Documents/Projects/specification.md"));
        assert!(!tree.contains("/Documents/Projects/spec.md"));

        // Rename directory with children
        let new_dir = tree.rename("/Documents", "Archive").unwrap();
        assert_eq!(new_dir, "/Archive");
        assert!(tree.contains("/Archive"));
        assert!(tree.contains("/Archive/Projects"));
        assert!(tree.contains("/Archive/Projects/specification.md"));
        assert!(!tree.contains("/Documents"));
        assert!(!tree.contains("/Documents/Projects"));

        let archive_children = tree.children("/Archive");
        assert_eq!(archive_children.len(), 1);
        assert_eq!(archive_children[0].path(), "/Archive/Projects");

        let project_children = tree.children("/Archive/Projects");
        assert_eq!(project_children.len(), 1);
        assert_eq!(
            project_children[0].path(),
            "/Archive/Projects/specification.md"
        );
    }

    #[test]
    fn move_node_and_cycle_prevention() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/FolderA",
        )));
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/FolderB",
        )));
        tree.insert(VirtualNode::new_file(NodeMetadata::file(
            "/FolderA/file.txt",
        )));

        // Cannot move root
        assert_eq!(
            tree.move_node("/", "/FolderB"),
            Err(VfsError::CannotModifyRoot)
        );

        // Cannot move into non-existent or file
        assert!(matches!(
            tree.move_node("/FolderA/file.txt", "/NonExistent"),
            Err(VfsError::NotFound(_))
        ));

        // Cannot move into self or descendant
        assert!(matches!(
            tree.move_node("/FolderA", "/FolderA"),
            Err(VfsError::CannotMoveIntoSelf(_))
        ));

        // Move file into FolderB
        let moved_file = tree.move_node("/FolderA/file.txt", "/FolderB").unwrap();
        assert_eq!(moved_file, "/FolderB/file.txt");
        assert!(tree.contains("/FolderB/file.txt"));
        assert!(!tree.contains("/FolderA/file.txt"));
        assert_eq!(tree.children("/FolderA").len(), 0);
        assert_eq!(tree.children("/FolderB").len(), 1);

        // Move FolderA into FolderB
        let moved_dir = tree.move_node("/FolderA", "/FolderB").unwrap();
        assert_eq!(moved_dir, "/FolderB/FolderA");
        assert!(tree.contains("/FolderB/FolderA"));
        assert!(!tree.contains("/FolderA"));
    }

    #[test]
    fn populate_from_resources_creates_hierarchy() {
        let mut tree = VirtualTree::new("root");
        let resources = vec![
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Folder/Sub/file.pdf",
                false,
            )
            .with_content_length(5000)
            .with_content_type("application/pdf")
            .with_etag("etag-123"),
        ];

        let count = tree.populate_from_resources("/remote.php/dav/files/user", &resources);
        assert_eq!(count, 1);
        assert!(tree.contains("/Folder"));
        assert!(tree.contains("/Folder/Sub"));
        assert!(tree.contains("/Folder/Sub/file.pdf"));
        let node = tree.get("/Folder/Sub/file.pdf").unwrap();
        assert_eq!(node.size(), 5000);
        assert_eq!(node.metadata.etag.as_deref(), Some("etag-123"));
    }

    #[test]
    fn populate_from_resources_merges_without_losing_existing_nodes() {
        let mut tree = VirtualTree::new("root");
        // Step 1: populate root with Documents and Photos
        let root_resources = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/", true),
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Documents", true)
                .with_content_length(1000),
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Photos", true)
                .with_content_length(2000),
        ];
        let c1 = tree.populate_from_resources("/remote.php/dav/files/user", &root_resources);
        assert_eq!(c1, 2);
        assert_eq!(tree.children("/").len(), 2);

        // Step 2: populate /Documents subfolder with a file
        let doc_resources = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Documents/", true),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Documents/report.pdf",
                false,
            )
            .with_content_length(500),
        ];
        let c2 = tree.populate_from_resources("/remote.php/dav/files/user", &doc_resources);
        assert_eq!(c2, 2);

        // Verify root still has both Documents and Photos
        assert_eq!(tree.children("/").len(), 2);
        assert!(tree.contains("/Photos"));
        assert!(tree.contains("/Documents"));
        assert!(tree.contains("/Documents/report.pdf"));

        // Verify directory metrics
        let doc_summary = tree.directory_size_summary("/Documents");
        assert_eq!(doc_summary.direct_items(), 1);
        assert_eq!(doc_summary.total_size, 500);

        // Photos was not populated with children yet, but carries server size
        let photo_summary = tree.directory_size_summary("/Photos");
        assert_eq!(photo_summary.total_size, 2000);

        // Verify populated status tracking
        assert!(tree.is_directory_populated("/"));
        assert!(tree.is_directory_populated("/Documents"));
        assert!(!tree.is_directory_populated("/Photos"));
        assert_eq!(tree.unpopulated_directories(), vec!["/Photos"]);
    }

    #[test]
    fn populate_from_resources_prunes_moved_or_deleted_items() {
        let mut tree = VirtualTree::new("root");
        let prefix = "/remote.php/dav/files/user";

        // Initial state: /Documents has fileA and fileB
        let initial = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Documents/", true),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Documents/fileA.txt",
                false,
            ),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Documents/fileB.txt",
                false,
            ),
        ];
        tree.populate_from_resources(prefix, &initial);
        assert!(tree.contains("/Documents/fileA.txt"));
        assert!(tree.contains("/Documents/fileB.txt"));

        // Server move: fileA is moved away, only fileB remains in /Documents
        let updated = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Documents/", true),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Documents/fileB.txt",
                false,
            ),
        ];
        tree.populate_from_resources(prefix, &updated);

        assert!(!tree.contains("/Documents/fileA.txt"));
        assert!(tree.contains("/Documents/fileB.txt"));
    }

    #[test]
    fn recursive_directory_size_and_summaries() {
        let mut tree = VirtualTree::new("Nextcloud");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory("/Docs")));
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Docs/Work",
        )));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Docs/readme.txt").with_size(100),
        ));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Docs/Work/report.pdf").with_size(500),
        ));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Docs/Work/data.csv").with_size(400),
        ));

        // Test directory_size_summary on nested folder
        let work_sum = tree.directory_size_summary("/Docs/Work");
        assert_eq!(work_sum.total_size, 900);
        assert_eq!(work_sum.file_count, 2);
        assert_eq!(work_sum.directory_count, 0);

        // Test directory_size on parent folder
        assert_eq!(tree.directory_size("/Docs"), 1000);
        let docs_sum = tree.directory_size_summary("/Docs");
        assert_eq!(docs_sum.total_size, 1000);
        assert_eq!(docs_sum.file_count, 3);
        assert_eq!(docs_sum.directory_count, 1);
        assert_eq!(docs_sum.items_description(), "3 files, 1 folder");

        // Test children_summaries on root
        let root_children = tree.children_summaries("/");
        assert_eq!(root_children.len(), 1);
        assert_eq!(root_children[0].name, "Docs");
        assert_eq!(root_children[0].size, 1000);
        assert_eq!(
            root_children[0].display_size(),
            "1000 B (3 files, 1 folder)"
        );

        // Test entry_summary on single path
        let work_entry = tree.entry_summary("/Docs/Work").unwrap();
        assert_eq!(work_entry.size, 900);
        assert_eq!(work_entry.display_size(), "900 B (2 files)");
    }

    #[test]
    fn populate_from_resources_handles_absolute_urls_and_percent_encoding() {
        let mut tree = VirtualTree::new("root");
        let prefix = "/remote.php/dav/files/alice";
        let resources = vec![
            crate::resource::DavResourceSummary::new(
                "https://cloud.example.com/remote.php/dav/files/alice/",
                true,
            ),
            crate::resource::DavResourceSummary::new(
                "https://cloud.example.com/remote.php/dav/files/alice/My%20Documents/",
                true,
            ),
            crate::resource::DavResourceSummary::new(
                "https://cloud.example.com/remote.php/dav/files/alice/My%20Documents/Work%20Plan.pdf",
                false,
            )
            .with_content_length(2048),
        ];

        let inserted = tree.populate_from_resources(prefix, &resources);
        assert_eq!(inserted, 2);
        assert!(tree.is_directory_populated("/"));

        let root_children = tree.children_summaries("/");
        assert_eq!(root_children.len(), 1);
        assert_eq!(root_children[0].path, "/My Documents");
        assert_eq!(root_children[0].name, "My Documents");
        assert!(root_children[0].is_directory);

        let sub_children = tree.children_summaries("/My Documents");
        assert_eq!(sub_children.len(), 1);
        assert_eq!(sub_children[0].path, "/My Documents/Work Plan.pdf");
        assert_eq!(sub_children[0].name, "Work Plan.pdf");
        assert_eq!(sub_children[0].size, 2048);
        assert!(!sub_children[0].is_directory);
    }

    /// Regression: a `Path`-backed hydration must be readable, not reported as "no data".
    #[test]
    fn read_hydrated_bytes_reads_through_cache_path() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg").with_size(4),
        ));

        let dir = std::env::temp_dir().join(format!("nexia-vfs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cached = dir.join("a.jpg");
        std::fs::write(&cached, b"DATA").unwrap();

        tree.hydrate_path("/Photos/a.jpg", cached.clone()).unwrap();

        // The legacy accessor silently reports nothing for path-backed nodes.
        assert!(
            tree.get("/Photos/a.jpg")
                .unwrap()
                .hydration()
                .bytes()
                .is_none()
        );

        assert_eq!(
            tree.read_hydrated_bytes("/Photos/a.jpg")
                .unwrap()
                .expect("path-backed hydration must be readable"),
            b"DATA".to_vec()
        );

        let _ = std::fs::remove_file(&cached);
        let _ = std::fs::remove_dir(&dir);
    }

    /// Regression: a resource refresh must not drop an already hydrated payload.
    #[test]
    fn refreshing_resources_keeps_hydrated_bytes() {
        let mut tree = VirtualTree::new("root");
        let prefix = "/remote.php/dav/files/user";
        let resources = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Photos/", true)
                .with_etag("etag-1".to_string()),
            crate::resource::DavResourceSummary {
                content_length: Some(4),
                ..crate::resource::DavResourceSummary::new(
                    "/remote.php/dav/files/user/Photos/a.jpg",
                    false,
                )
                .with_etag("etag-1".to_string())
            },
        ];
        tree.populate_from_resources(prefix, &resources);
        tree.hydrate("/Photos/a.jpg", b"DATA".to_vec()).unwrap();

        // Same revision reported again (a periodic ETag poll): hydration must survive.
        tree.populate_from_resources(prefix, &resources);
        assert_eq!(
            tree.read_hydrated_bytes("/Photos/a.jpg").unwrap().unwrap(),
            b"DATA".to_vec()
        );

        // A new revision must NOT keep the stale payload.
        let changed = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Photos/", true)
                .with_etag("etag-2".to_string()),
            crate::resource::DavResourceSummary {
                content_length: Some(4),
                ..crate::resource::DavResourceSummary::new(
                    "/remote.php/dav/files/user/Photos/a.jpg",
                    false,
                )
                .with_etag("etag-2".to_string())
            },
        ];
        tree.populate_from_resources(prefix, &changed);
        assert!(tree.get("/Photos/a.jpg").unwrap().is_placeholder());
    }

    /// A live mount's hydration state wins over the snapshot it is merged into.
    #[test]
    fn adopt_hydration_from_live_tree() {
        let mut live = VirtualTree::new("root");
        live.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg")
                .with_size(4)
                .with_etag("etag-1".to_string()),
        ));
        live.hydrate("/Photos/a.jpg", b"DATA".to_vec()).unwrap();

        // Snapshot taken before the file was hydrated.
        let mut snapshot = VirtualTree::new("root");
        snapshot.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg")
                .with_size(4)
                .with_etag("etag-1".to_string()),
        ));
        assert_eq!(snapshot.adopt_hydration_from(&live), 1);
        assert!(!snapshot.get("/Photos/a.jpg").unwrap().is_placeholder());

        // A different revision must not inherit the payload.
        let mut stale = VirtualTree::new("root");
        stale.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg")
                .with_size(4)
                .with_etag("etag-9".to_string()),
        ));
        assert_eq!(stale.adopt_hydration_from(&live), 0);
        assert!(stale.get("/Photos/a.jpg").unwrap().is_placeholder());
    }

    /// A partial refresh must not prune siblings it never asked about.
    #[test]
    fn merge_from_resources_never_prunes() {
        let mut tree = VirtualTree::new("root");
        let prefix = "/remote.php/dav/files/user";
        let initial = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Photos/", true),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Photos/a.jpg",
                false,
            ),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Photos/b.jpg",
                false,
            ),
        ];
        tree.populate_from_resources(prefix, &initial);

        let partial = vec![
            crate::resource::DavResourceSummary::new("/remote.php/dav/files/user/Photos/", true),
            crate::resource::DavResourceSummary::new(
                "/remote.php/dav/files/user/Photos/b.jpg",
                false,
            ),
        ];
        tree.merge_from_resources(prefix, &partial);

        assert!(tree.contains("/Photos/a.jpg"));
        assert!(tree.contains("/Photos/b.jpg"));
    }

    /// Regression: a persisted tree must not embed file payloads.
    ///
    /// One account's metadata-only tree reached 483 MB for 1046 files because in-memory payloads
    /// were written as JSON number arrays, so every save and every start-up parsed half a gigabyte.
    #[test]
    fn persisted_tree_omits_payloads_and_thumbnails() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg").with_size(5),
        ));
        tree.hydrate("/Photos/a.jpg", b"hello".to_vec()).unwrap();
        tree.set_thumbnail("/Photos/a.jpg", 256, 256, vec![0xAB; 64])
            .unwrap();

        let json = serde_json::to_string(&tree).unwrap();

        assert!(
            !json.contains("hello"),
            "payload leaked into the persisted tree: {json}"
        );
        assert!(
            !json.contains("thumbnails"),
            "thumbnails leaked into the persisted tree: {json}"
        );
        assert!(
            json.len() < 1024,
            "metadata-only tree was {} bytes",
            json.len()
        );

        // A payload that was never spilled degrades to a placeholder instead of bloating the file.
        let reloaded: VirtualTree = serde_json::from_str(&json).unwrap();
        assert!(reloaded.get("/Photos/a.jpg").unwrap().is_placeholder());
    }

    /// Spilling writes the payload to the cache and leaves a path in the tree, so a restart reuses
    /// the downloaded content instead of fetching it again.
    #[test]
    fn spill_hydrated_bytes_writes_content_and_persists_path() {
        let mut tree = VirtualTree::new("root");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Photos",
        )));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Photos/a.jpg").with_size(5),
        ));
        tree.hydrate("/Photos/a.jpg", b"hello".to_vec()).unwrap();

        let root = std::env::temp_dir().join(format!("nexia-spill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(tree.spill_hydrated_bytes(&root).unwrap(), 1);

        let cached = root.join("Photos/a.jpg");
        assert_eq!(std::fs::read(&cached).unwrap(), b"hello".to_vec());
        assert_eq!(
            tree.get("/Photos/a.jpg").unwrap().hydration().cached_path(),
            Some(cached.as_path())
        );

        // The tree now persists a path, not the payload, and reloading keeps it hydrated.
        let json = serde_json::to_string(&tree).unwrap();
        assert!(!json.contains("hello"), "payload leaked: {json}");
        let reloaded: VirtualTree = serde_json::from_str(&json).unwrap();
        assert_eq!(
            reloaded
                .read_hydrated_bytes("/Photos/a.jpg")
                .unwrap()
                .unwrap(),
            b"hello".to_vec()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Spilling must not write outside the cache root, even for a hostile node path.
    #[test]
    fn spill_hydrated_bytes_ignores_escaping_paths() {
        let mut tree = VirtualTree::new("root");
        let mut node = VirtualNode::new_file(NodeMetadata::file("/a.jpg").with_size(5));
        node.metadata.path = "/../../etc/passwd".to_string();
        node.hydration = HydrationState::Hydrated(HydratedContent::Bytes(b"hello".to_vec()));
        tree.insert(node);

        let root = std::env::temp_dir().join(format!("nexia-spill-esc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(tree.spill_hydrated_bytes(&root).unwrap(), 0);
        assert!(!root.exists() || std::fs::read_dir(&root).unwrap().next().is_none());

        let _ = std::fs::remove_dir_all(&root);
    }
}
