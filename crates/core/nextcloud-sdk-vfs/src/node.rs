//! Virtual node representation within the tree.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::hydration::HydrationState;
use crate::metadata::NodeMetadata;
use crate::thumbnail::ThumbnailCache;

/// A node in the virtual file system tree representing a file or directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualNode {
    /// Metadata describing the node.
    pub metadata: NodeMetadata,
    /// Current hydration state of the file content.
    pub hydration: HydrationState,
    /// Cached thumbnails keyed by resolution.
    pub thumbnails: ThumbnailCache,
    /// Canonical paths of direct children if this node is a directory.
    pub children: Option<BTreeSet<String>>,
}

impl VirtualNode {
    /// Constructs a virtual file node in placeholder state.
    pub fn new_file(metadata: NodeMetadata) -> Self {
        Self {
            metadata,
            hydration: HydrationState::Placeholder,
            thumbnails: ThumbnailCache::new(),
            children: None,
        }
    }

    /// Constructs a virtual directory node with an empty children set.
    pub fn new_directory(mut metadata: NodeMetadata) -> Self {
        metadata.is_directory = true;
        Self {
            metadata,
            hydration: HydrationState::Placeholder,
            thumbnails: ThumbnailCache::new(),
            children: Some(BTreeSet::new()),
        }
    }

    /// Sets the hydration state for this node.
    #[must_use]
    pub fn with_hydration(mut self, hydration: impl Into<HydrationState>) -> Self {
        self.hydration = hydration.into();
        self
    }

    /// Caches a thumbnail resolution for this node.
    #[must_use]
    pub fn with_thumbnail(mut self, width: u32, height: u32, bytes: Vec<u8>) -> Self {
        self.thumbnails.insert(width, height, bytes);
        self
    }

    /// Returns a reference to the node metadata.
    pub const fn metadata(&self) -> &NodeMetadata {
        &self.metadata
    }

    /// Returns a mutable reference to the node metadata.
    pub fn metadata_mut(&mut self) -> &mut NodeMetadata {
        &mut self.metadata
    }

    /// Returns a reference to the current hydration state.
    pub const fn hydration(&self) -> &HydrationState {
        &self.hydration
    }

    /// Returns a mutable reference to the current hydration state.
    pub fn hydration_mut(&mut self) -> &mut HydrationState {
        &mut self.hydration
    }

    /// Returns `true` if this file is an unloaded placeholder.
    pub const fn is_placeholder(&self) -> bool {
        self.hydration.is_placeholder()
    }

    /// Returns `true` if content is currently hydrated.
    pub const fn is_hydrated(&self) -> bool {
        self.hydration.is_hydrated()
    }

    /// Returns `true` if the server reports preview generation is available.
    pub const fn has_preview(&self) -> bool {
        self.metadata.has_preview
    }

    /// Returns `true` if this node represents a directory.
    pub const fn is_directory(&self) -> bool {
        self.metadata.is_directory
    }

    /// Returns `true` if this node represents a file.
    pub const fn is_file(&self) -> bool {
        !self.metadata.is_directory
    }

    /// Returns the canonical path of the node.
    pub fn path(&self) -> &str {
        &self.metadata.path
    }

    /// Returns the display name of the node.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns the file size in bytes reported by metadata.
    pub const fn size(&self) -> u64 {
        self.metadata.size
    }

    /// Returns a reference to the thumbnail cache.
    pub const fn thumbnails(&self) -> &ThumbnailCache {
        &self.thumbnails
    }

    /// Returns a mutable reference to the thumbnail cache.
    pub fn thumbnails_mut(&mut self) -> &mut ThumbnailCache {
        &mut self.thumbnails
    }

    /// Returns the set of children paths if this node is a directory.
    pub const fn children_paths(&self) -> Option<&BTreeSet<String>> {
        self.children.as_ref()
    }

    /// Adds a child path to this directory node.
    pub fn add_child(&mut self, child_path: impl Into<String>) {
        if let Some(children) = &mut self.children {
            children.insert(child_path.into());
        }
    }

    /// Removes a child path from this directory node.
    pub fn remove_child(&mut self, child_path: &str) -> bool {
        self.children
            .as_mut()
            .is_some_and(|children| children.remove(child_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_node_placeholder_behavior() {
        let meta = NodeMetadata::file("/Photos/vacation.jpg")
            .with_size(2048)
            .with_has_preview(true);
        let node = VirtualNode::new_file(meta);

        assert!(node.is_file());
        assert!(!node.is_directory());
        assert!(node.is_placeholder());
        assert!(!node.is_hydrated());
        assert!(node.has_preview());
        assert_eq!(node.size(), 2048);
        assert_eq!(node.path(), "/Photos/vacation.jpg");
        assert_eq!(node.name(), "vacation.jpg");
        assert!(node.children_paths().is_none());
    }

    #[test]
    fn directory_node_children() {
        let meta = NodeMetadata::directory("/Photos");
        let mut node = VirtualNode::new_directory(meta);

        assert!(node.is_directory());
        assert!(!node.is_file());
        assert_eq!(node.children_paths().unwrap().len(), 0);

        node.add_child("/Photos/vacation.jpg");
        assert_eq!(node.children_paths().unwrap().len(), 1);
        assert!(
            node.children_paths()
                .unwrap()
                .contains("/Photos/vacation.jpg")
        );

        assert!(node.remove_child("/Photos/vacation.jpg"));
        assert_eq!(node.children_paths().unwrap().len(), 0);
    }
}
