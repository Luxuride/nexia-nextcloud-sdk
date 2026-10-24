//! Transient VFS FUSE driver for desktop filesystem mounting.
//!
//! Provides the core filesystem operations ([`FuseDriver`]) that bridge
//! an in-memory or transient [`VirtualTree`](sdk_vfs::VirtualTree) to OS FUSE mounts:
//! - Inode allocation and lookup ([`InodeTable`])
//! - Metadata resolution ([`FileAttr`])
//! - Directory enumeration ([`DirEntry`])
//! - On-demand payload hydration ([`AsyncHydrator`])
//! - Multi-resolution thumbnail exposure via extended attributes

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod attr;
pub mod driver;
pub mod entry;
pub mod hydrator;
pub mod inode;

pub use attr::{FileAttr, FileType};
pub use driver::FuseDriver;
pub use entry::DirEntry;
pub use hydrator::{AsyncHydrator, NoopHydrator};
pub use inode::{FUSE_ROOT_INODE, InodeTable};
