use serde::{Deserialize, Serialize};

use super::ChunkedUpload;
use crate::wire::{lossy_bool, lossy_opt_u64};

/// File-related capabilities advertised by the Files application.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct FilesCapabilities {
    /// Whether the server supports chunked uploads for large files.
    #[serde(
        default,
        alias = "bigfilechunking",
        alias = "big_file_chunking",
        deserialize_with = "lossy_bool"
    )]
    pub big_file_chunking: bool,
    /// Whether soft deletion and trashbin restoration are available.
    #[serde(default, deserialize_with = "lossy_bool")]
    pub undelete: bool,
    /// Whether file versioning history is supported.
    #[serde(default, deserialize_with = "lossy_bool")]
    pub versioning: bool,
    /// Explicit chunked upload settings block, if present.
    #[serde(default, alias = "chunked_upload", alias = "chunkedUpload")]
    pub chunked_upload: Option<ChunkedUpload>,
    /// Direct max chunk size advertisement in bytes, if present.
    #[serde(
        default,
        alias = "max_chunk_size",
        alias = "maxChunkSize",
        deserialize_with = "lossy_opt_u64"
    )]
    pub max_chunk_size: Option<u64>,
}

impl FilesCapabilities {
    /// Whether big file chunking is supported.
    pub const fn big_file_chunking(&self) -> bool {
        self.big_file_chunking
    }

    /// Whether trashbin / undelete functionality is enabled.
    pub const fn undelete(&self) -> bool {
        self.undelete
    }

    /// Whether file versioning is enabled.
    pub const fn versioning(&self) -> bool {
        self.versioning
    }

    /// Chunked upload configuration, if provided by the server.
    pub fn chunked_upload(&self) -> Option<&ChunkedUpload> {
        self.chunked_upload.as_ref()
    }

    /// Effective maximum upload chunk size in bytes, if advertised directly
    /// or inside the [`ChunkedUpload`] block.
    pub fn max_chunk_size(&self) -> Option<u64> {
        self.max_chunk_size
            .or_else(|| self.chunked_upload.as_ref().and_then(|c| c.max_size))
    }
}
