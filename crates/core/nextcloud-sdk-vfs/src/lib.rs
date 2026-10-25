//! Transient virtual file system mounting layer for the Nextcloud Rust SDK.
//!
//! This crate provides an in-memory virtual file system representation that
//! models the remote Nextcloud hierarchical tree, rich file metadata, hydration
//! state (tracking unloaded placeholders versus loaded payloads), and
//! multi-resolution thumbnail caching without eagerly downloading files.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod category;
pub mod dir_size;
pub mod entry;
pub mod error;
pub mod hydration;
pub mod metadata;
pub mod navigation;
pub mod node;
pub mod path;
pub mod permissions;
pub mod resource;
pub mod seed;
pub mod thumbnail;
pub mod tree;

pub use category::{ResourceIconCategory, format_bytes};
pub use dir_size::DirectorySizeSummary;
pub use entry::VirtualEntrySummary;
pub use error::VfsError;
pub use hydration::{HydratedContent, HydrationState};
pub use metadata::NodeMetadata;
pub use navigation::{BreadcrumbSegment, NavigationHistory, breadcrumb_segments};
pub use node::VirtualNode;
pub use path::{file_name, normalize_path, parent_path};
pub use permissions::NextcloudPermissions;
pub use resource::DavResourceSummary;
pub use seed::{HierarchySeeder, StandardNextcloudSeeder};
pub use thumbnail::ThumbnailCache;
pub use tree::VirtualTree;
