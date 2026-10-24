//! In-memory hierarchical virtual file system tree.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
}
