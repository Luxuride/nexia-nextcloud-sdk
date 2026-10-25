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

    /// Optional notification when directory entries are read in FUSE, allowing prefetching previews.
    fn on_readdir(&self, _paths: &[String]) {}
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
