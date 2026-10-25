//! Virtual file system entry summary model.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::category::{ResourceIconCategory, format_bytes};
use crate::dir_size::DirectorySizeSummary;
use crate::node::VirtualNode;
use crate::permissions::NextcloudPermissions;
use crate::tree::VirtualTree;

/// Summary representation of a virtual file or directory entry for UI display and client consumers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualEntrySummary {
    /// Canonical relative path within the virtual mount (e.g. `"/Photos/cat.jpg"`).
    pub path: String,
    /// Display file or folder name.
    pub name: String,
    /// Whether this entry represents a directory.
    pub is_directory: bool,
    /// File size in bytes, or aggregated recursive size for directories when computed with tree context.
    pub size: u64,
    /// ETag identifier.
    pub etag: Option<String>,
    /// Last modification timestamp as formatted string.
    pub modified_at: Option<String>,
    /// Permission flags string reported by Nextcloud (e.g. `"RDNVW"`).
    pub permissions: Option<String>,
    /// Content MIME type.
    pub mime_type: Option<String>,
    /// Nextcloud internal numeric file ID (`oc:fileid`), if reported by the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<u64>,
    /// Semantic Freedesktop icon category determined by the SDK.
    pub icon_category: ResourceIconCategory,
    /// Whether the file is currently a metadata-only placeholder or hydrated.
    pub is_hydrated: bool,
    /// Local disk cache path if hydrated.
    pub cached_path: Option<PathBuf>,
    /// Aggregated directory contents summary if this entry is a directory and computed with tree context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory_summary: Option<DirectorySizeSummary>,
}

impl VirtualEntrySummary {
    /// Constructs a summary for `node`, incorporating recursive directory size and item counts from `tree`.
    pub fn from_node_with_tree(node: &VirtualNode, tree: &VirtualTree) -> Self {
        let mut summary = Self::from(node);
        if node.is_directory() {
            let dir_sum = tree.directory_size_summary(&node.metadata.path);
            summary.size = dir_sum.total_size;
            summary.directory_summary = Some(dir_sum);
        }
        summary
    }

    /// Decodes the raw Nextcloud permissions string into typed capabilities, if available.
    pub fn parsed_permissions(&self) -> Option<NextcloudPermissions> {
        self.permissions.as_deref().map(NextcloudPermissions::parse)
    }

    /// Formats a human-friendly size representation.
    ///
    /// For files, returns formatted byte size (e.g. `"2.4 MB"`).
    /// For directories with a computed summary, returns detailed summary (e.g. `"14.2 MB (28 files, 3 folders)"` or `"Empty folder"`).
    /// For directories without a computed summary, returns `"Folder"` if size is 0 or formatted bytes if non-zero.
    pub fn display_size(&self) -> String {
        if self.is_directory {
            if let Some(ref dir_sum) = self.directory_summary {
                dir_sum.detailed_summary()
            } else if self.size > 0 {
                format_bytes(self.size)
            } else {
                "Folder".to_string()
            }
        } else {
            format_bytes(self.size)
        }
    }
}

impl From<&VirtualNode> for VirtualEntrySummary {
    fn from(node: &VirtualNode) -> Self {
        Self {
            path: node.metadata.path.clone(),
            name: node.metadata.name.clone(),
            is_directory: node.metadata.is_directory,
            size: node.metadata.size,
            etag: node.metadata.etag.clone(),
            modified_at: node.metadata.modified_at.clone(),
            permissions: node.metadata.permissions.clone(),
            mime_type: node.metadata.mime_type.clone(),
            file_id: node.metadata.file_id,
            icon_category: node.metadata.icon_category(),
            is_hydrated: node.hydration.is_hydrated(),
            cached_path: node.hydration.cached_path().map(|p| p.to_path_buf()),
            directory_summary: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hydration::{HydratedContent, HydrationState};
    use crate::metadata::NodeMetadata;

    #[test]
    fn summary_from_virtual_node() {
        let meta = NodeMetadata::file("/docs/spec.pdf")
            .with_size(4096)
            .with_mime_type("application/pdf")
            .with_permissions("RDNVW")
            .with_etag("\"xyz\"");

        let mut node = VirtualNode::new_file(meta);
        let summary = VirtualEntrySummary::from(&node);
        assert_eq!(summary.path, "/docs/spec.pdf");
        assert_eq!(summary.name, "spec.pdf");
        assert!(!summary.is_directory);
        assert_eq!(summary.size, 4096);
        assert_eq!(summary.display_size(), "4.0 KB");
        assert!(!summary.is_hydrated);
        assert_eq!(summary.icon_category, ResourceIconCategory::Document);

        let perms = summary.parsed_permissions().unwrap();
        assert!(perms.can_read());
        assert!(perms.can_write());
        assert_eq!(perms.summary_label(), "Read & Write (Full)");

        node.hydration =
            HydrationState::Hydrated(HydratedContent::Path(PathBuf::from("/cache/spec.pdf")));
        let summary_hydrated = VirtualEntrySummary::from(&node);
        assert!(summary_hydrated.is_hydrated);
        assert_eq!(
            summary_hydrated.cached_path,
            Some(PathBuf::from("/cache/spec.pdf"))
        );
    }

    #[test]
    fn summary_with_tree_directory_size() {
        let mut tree = VirtualTree::new("Nextcloud");
        tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
            "/Documents",
        )));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Documents/doc1.txt").with_size(1024),
        ));
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file("/Documents/doc2.txt").with_size(2048),
        ));

        let doc_node = tree.get("/Documents").unwrap();
        let summary = VirtualEntrySummary::from_node_with_tree(doc_node, &tree);

        assert!(summary.is_directory);
        assert_eq!(summary.size, 3072);
        assert_eq!(summary.display_size(), "3.0 KB (2 files)");
        assert_eq!(
            summary.directory_summary,
            Some(
                DirectorySizeSummary::new(3072, 2, 0)
                    .with_sync_state(true, true)
                    .with_direct(3072, 2, 0)
            )
        );
    }
}
