use serde::{Deserialize, Serialize};

use crate::wire::lossy_opt_u64;

/// Configuration parameters for chunked file uploads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChunkedUpload {
    /// Maximum allowable size in bytes for an individual upload chunk.
    #[serde(
        default,
        alias = "max_size",
        alias = "maxSize",
        deserialize_with = "lossy_opt_u64"
    )]
    pub max_size: Option<u64>,
    /// Maximum number of upload chunks that may be sent concurrently.
    #[serde(default, alias = "max_parallel_count", alias = "maxParallelCount")]
    pub max_parallel_count: Option<u32>,
}

impl ChunkedUpload {
    /// Maximum chunk size limit in bytes.
    pub const fn max_size(&self) -> Option<u64> {
        self.max_size
    }

    /// Maximum concurrent chunk upload count.
    pub const fn max_parallel_count(&self) -> Option<u32> {
        self.max_parallel_count
    }
}
