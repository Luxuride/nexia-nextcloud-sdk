//! Trait and default implementation for seeding initial virtual file hierarchies.

use crate::metadata::NodeMetadata;
use crate::node::VirtualNode;
use crate::tree::VirtualTree;

/// Behaviour for populating an initial placeholder hierarchy into a [`VirtualTree`].
pub trait HierarchySeeder: Send + Sync + 'static {
    /// Populates `tree` with initial directories and placeholder files, returning the inserted count.
    fn seed(tree: &mut VirtualTree) -> usize;
}

/// Seeds standard Nextcloud starter folders and sample placeholder files.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandardNextcloudSeeder;

impl HierarchySeeder for StandardNextcloudSeeder {
    fn seed(tree: &mut VirtualTree) -> usize {
        let dirs = [
            "/Documents",
            "/Documents/Projects",
            "/Photos",
            "/Music",
            "/Templates",
        ];
        for d in dirs {
            tree.insert(VirtualNode::new_directory(NodeMetadata::directory(d)));
        }

        let files: &[(&str, u64, &str)] = &[
            ("/Nextcloud Manual.pdf", 4_820_400, "application/pdf"),
            ("/Notes.md", 3_420, "text/markdown"),
            (
                "/Documents/Architecture Overview.pdf",
                1_945_600,
                "application/pdf",
            ),
            (
                "/Documents/Q4 Budget 2026.ods",
                86_016,
                "application/vnd.oasis.opendocument.spreadsheet",
            ),
            (
                "/Documents/Keynote Deck.odp",
                6_291_456,
                "application/vnd.oasis.opendocument.presentation",
            ),
            ("/Documents/Projects/Roadmap.md", 12_288, "text/markdown"),
            (
                "/Documents/Projects/Release Bundle.zip",
                18_450_944,
                "application/zip",
            ),
            ("/Photos/Mountain Sunrise.jpg", 3_670_016, "image/jpeg"),
            ("/Photos/Nextcloud Conference.png", 2_150_400, "image/png"),
            ("/Music/Focus Ambient.flac", 24_117_248, "audio/flac"),
            (
                "/Templates/Meeting Notes.odt",
                28_672,
                "application/vnd.oasis.opendocument.text",
            ),
        ];

        for &(path, size, mime) in files {
            let meta = NodeMetadata::file(path)
                .with_size(size)
                .with_mime_type(mime.to_string());
            tree.insert(VirtualNode::new_file(meta));
        }

        dirs.len() + files.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_seeder_populates_hierarchy() {
        let mut tree = VirtualTree::new("Nextcloud");
        let count = StandardNextcloudSeeder::seed(&mut tree);
        assert_eq!(count, 16);
        assert!(tree.contains("/Documents"));
        assert!(tree.contains("/Documents/Projects/Roadmap.md"));
        assert!(tree.contains("/Photos/Mountain Sunrise.jpg"));
    }
}
