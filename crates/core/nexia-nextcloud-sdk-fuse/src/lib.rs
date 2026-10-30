//! Transient VFS FUSE driver for desktop filesystem mounting.
//!
//! Provides the core filesystem operations ([`FuseDriver`]) that bridge
//! an in-memory or transient [`VirtualTree`](nexia_nextcloud_sdk_vfs::VirtualTree) to OS FUSE mounts:
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
#[cfg(target_os = "linux")]
pub mod fs;
pub mod hydrator;
pub mod inode;
pub mod intent;

pub use attr::{FileAttr, FileType};
pub use driver::{
    DownloadGate, DownloadGuard, FULL_HYDRATION_MAX_SIZE, FuseDriver, HYDRATION_BLOCK_SIZE,
    MAX_INFLIGHT_DOWNLOADS, RANGED_WINDOW_SIZE, parse_date_to_unix_secs,
};
pub use entry::DirEntry;
#[cfg(target_os = "linux")]
pub use fs::{mount_virtual_fs, mount_virtual_fs_with_options};
#[cfg(target_os = "linux")]
pub use fuser::{BackgroundSession, MountOption};
pub use hydrator::{AsyncHydrator, NoopHydrator};
pub use inode::{FUSE_ROOT_INODE, InodeTable};
pub use intent::{ReadIntent, ReadVerdict};

#[cfg(not(target_os = "linux"))]
#[derive(Clone, Debug)]
/// Stub background session on platforms without native FUSE mounting support.
pub struct BackgroundSession;

#[cfg(not(target_os = "linux"))]
#[derive(Clone, Debug)]
/// Stub mount option on platforms without native FUSE mounting support.
pub enum MountOption {}

#[cfg(not(target_os = "linux"))]
/// Mounts a virtual filesystem with FUSE.
///
/// On non-Linux platforms where native FUSE is unavailable, this returns `std::io::ErrorKind::Unsupported`.
pub fn mount_virtual_fs<P: AsRef<std::path::Path>>(
    _driver: driver::FuseDriver,
    _mountpoint: P,
) -> std::io::Result<BackgroundSession> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Native FUSE mounting is only supported on Linux",
    ))
}

#[cfg(not(target_os = "linux"))]
/// Mounts a virtual filesystem with FUSE and custom options.
///
/// On non-Linux platforms where native FUSE is unavailable, this returns `std::io::ErrorKind::Unsupported`.
pub fn mount_virtual_fs_with_options<P: AsRef<std::path::Path>>(
    _driver: driver::FuseDriver,
    _mountpoint: P,
    _options: Vec<MountOption>,
) -> std::io::Result<BackgroundSession> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Native FUSE mounting is only supported on Linux",
    ))
}
