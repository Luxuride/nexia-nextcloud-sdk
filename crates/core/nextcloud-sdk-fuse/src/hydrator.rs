//! On-demand payload hydrator seam for FUSE read operations.

use std::future::Future;
use std::pin::Pin;

use nextcloud_sdk_vfs::VfsError;

/// Trait implemented by components that fetch remote file payloads on demand.
pub trait AsyncHydrator: Send + Sync {
    /// Hydrates remote file content for `canonical_path`, returning the raw bytes.
    fn hydrate(
        &self,
        canonical_path: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>>;

    /// On-demand partial byte range hydration for placeholder reads.
    ///
    /// By default, falls back to full hydration.
    fn hydrate_range(
        &self,
        canonical_path: &str,
        offset: u64,
        size: u32,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>> {
        let path = canonical_path.to_string();
        Box::pin(async move {
            let full = self.hydrate(&path).await?;
            let start = (offset as usize).min(full.len());
            let end = (start + size as usize).min(full.len());
            Ok(full[start..end].to_vec())
        })
    }

    /// Optional notification when directory entries are read in FUSE, allowing prefetching previews.
    fn on_readdir(&self, _paths: &[String]) {}

    /// Optional notification when a directory path is being read in FUSE, allowing on-demand crawl.
    fn on_directory_read(&self, _path: &str) {}

    /// Synchronously or cooperatively fetches and populates remote directory children if unpopulated.
    fn hydrate_directory(
        &self,
        _canonical_path: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), VfsError>> + Send + '_>> {
        Box::pin(async move { Ok(()) })
    }
}

/// A no-op hydrator that always returns [`VfsError::NotFound`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopHydrator;

impl AsyncHydrator for NoopHydrator {
    fn hydrate(
        &self,
        canonical_path: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>> {
        let err = VfsError::NotFound(canonical_path.to_string());
        Box::pin(async move { Err(err) })
    }
}
