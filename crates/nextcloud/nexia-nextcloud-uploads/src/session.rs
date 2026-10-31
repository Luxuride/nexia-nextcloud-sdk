//! State and chunk boundary calculations for Nextcloud chunked uploads.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};

/// Minimum chunk size recommended by Nextcloud (5 MiB).
pub const MIN_CHUNK_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum chunk size recommended by Nextcloud (5 GiB).
pub const MAX_CHUNK_SIZE: u64 = 5 * 1024 * 1024 * 1024;

/// Default chunk size used for uploads (10 MiB).
pub const DEFAULT_CHUNK_SIZE: u64 = 10 * 1024 * 1024;

/// Describes the byte boundaries of an individual upload chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkBoundary {
    /// 1-based index of this chunk.
    pub index: u32,
    /// Inclusive byte offset where this chunk starts within the upload stream.
    pub start: u64,
    /// Exclusive byte offset where this chunk ends within the upload stream.
    pub end: u64,
}

impl ChunkBoundary {
    /// Constructs a new chunk boundary.
    pub const fn new(index: u32, start: u64, end: u64) -> Self {
        Self { index, start, end }
    }

    /// Size of this chunk in bytes.
    pub const fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether this chunk has zero length.
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Active chunked upload session state.
///
/// Keeps track of user ID, upload ID, destination path, total file size,
/// and which chunks have been uploaded successfully.
#[derive(Clone, Debug)]
pub struct UploadSession {
    user_id: String,
    upload_id: String,
    destination_path: String,
    total_size: u64,
    uploaded_chunks: Arc<Mutex<BTreeSet<u32>>>,
}

impl UploadSession {
    /// Minimum chunk size recommended by Nextcloud (5 MiB).
    pub const MIN_CHUNK_SIZE: u64 = MIN_CHUNK_SIZE;
    /// Maximum chunk size recommended by Nextcloud (5 GiB).
    pub const MAX_CHUNK_SIZE: u64 = MAX_CHUNK_SIZE;
    /// Default chunk size used for uploads (10 MiB).
    pub const DEFAULT_CHUNK_SIZE: u64 = DEFAULT_CHUNK_SIZE;

    /// Creates a new upload session without any recorded chunks.
    pub fn new(
        user_id: impl Into<String>,
        upload_id: impl Into<String>,
        destination_path: impl Into<String>,
        total_size: u64,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            upload_id: upload_id.into(),
            destination_path: destination_path.into(),
            total_size,
            uploaded_chunks: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    /// User ID of the upload owner.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Unique identifier for this upload session.
    pub fn upload_id(&self) -> &str {
        &self.upload_id
    }

    /// Destination WebDAV path or URI where the assembled file will be placed.
    pub fn destination_path(&self) -> &str {
        &self.destination_path
    }

    /// Total size in bytes of the complete file being uploaded.
    pub const fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Whether `chunk_size` is inside the range Nextcloud accepts.
    ///
    /// Nothing enforces this, but uploading outside it produces a session the
    /// server assembles slowly or not at all.
    pub const fn is_supported_chunk_size(chunk_size: u64) -> bool {
        chunk_size >= MIN_CHUNK_SIZE && chunk_size <= MAX_CHUNK_SIZE
    }

    /// Records that a chunk index has been uploaded successfully.
    pub fn mark_chunk_uploaded(&self, chunk_index: u32) {
        self.lock_chunks().insert(chunk_index);
    }

    /// Checks whether a specific chunk index has been uploaded.
    pub fn is_chunk_uploaded(&self, chunk_index: u32) -> bool {
        self.lock_chunks().contains(&chunk_index)
    }

    /// Returns a set of all chunk indices uploaded so far.
    pub fn uploaded_chunks(&self) -> BTreeSet<u32> {
        self.lock_chunks().clone()
    }

    /// Number of chunks marked as successfully uploaded.
    pub fn uploaded_chunk_count(&self) -> usize {
        self.lock_chunks().len()
    }

    /// Calculates the expected total chunk count for the given chunk size.
    ///
    /// Returns `None` if `chunk_size` is zero or if total chunk count overflows `u32`.
    pub fn expected_chunk_count(&self, chunk_size: u64) -> Option<u32> {
        if chunk_size == 0 {
            return None;
        }
        if self.total_size == 0 {
            return Some(1);
        }
        let count = self.total_size.div_ceil(chunk_size);
        u32::try_from(count).ok()
    }

    /// Whether all expected chunks for the given chunk size have been uploaded.
    pub fn is_complete(&self, chunk_size: u64) -> bool {
        let Some(expected) = self.expected_chunk_count(chunk_size) else {
            return false;
        };
        let uploaded = self.lock_chunks();
        if uploaded.len() != expected as usize {
            return false;
        }
        (1..=expected).all(|i| uploaded.contains(&i))
    }

    /// Calculates the byte boundary for a single 1-based chunk index.
    ///
    /// Returns `None` if `chunk_size` is zero, `chunk_index` is zero, or exceeds total chunk count.
    pub fn chunk_boundary(&self, chunk_index: u32, chunk_size: u64) -> Option<ChunkBoundary> {
        if chunk_size == 0 || chunk_index == 0 {
            return None;
        }
        let total_chunks = self.expected_chunk_count(chunk_size)?;
        if chunk_index > total_chunks {
            return None;
        }
        let offset = u64::from(chunk_index - 1) * chunk_size;
        let end = offset.saturating_add(chunk_size).min(self.total_size);
        Some(ChunkBoundary::new(chunk_index, offset, end))
    }

    /// Calculates all chunk boundaries for this upload given a chunk size.
    ///
    /// Returns `None` if `chunk_size` is zero or if total chunk count overflows `u32`.
    pub fn chunk_boundaries(&self, chunk_size: u64) -> Option<Vec<ChunkBoundary>> {
        let total_chunks = self.expected_chunk_count(chunk_size)?;
        // A caller-supplied chunk size is not validated, so the reservation is
        // capped: a tiny chunk size would otherwise reserve one boundary per
        // byte of the file before the first one is calculated.
        let mut boundaries =
            Vec::with_capacity(usize::try_from(total_chunks).unwrap_or(0).min(1024));
        for index in 1..=total_chunks {
            if let Some(boundary) = self.chunk_boundary(index, chunk_size) {
                boundaries.push(boundary);
            }
        }
        Some(boundaries)
    }

    fn lock_chunks(&self) -> MutexGuard<'_, BTreeSet<u32>> {
        match self.uploaded_chunks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_calculations_exact_multiple() {
        let session = UploadSession::new("alice", "up-1", "/path", 30 * 1024 * 1024);
        let chunk_size = 10 * 1024 * 1024;
        assert_eq!(session.expected_chunk_count(chunk_size), Some(3));

        let boundaries = session.chunk_boundaries(chunk_size).unwrap();
        assert_eq!(boundaries.len(), 3);

        assert_eq!(boundaries[0].index, 1);
        assert_eq!(boundaries[0].start, 0);
        assert_eq!(boundaries[0].end, 10 * 1024 * 1024);
        assert_eq!(boundaries[0].len(), 10 * 1024 * 1024);
        assert!(!boundaries[0].is_empty());

        assert_eq!(boundaries[1].index, 2);
        assert_eq!(boundaries[1].start, 10 * 1024 * 1024);
        assert_eq!(boundaries[1].end, 20 * 1024 * 1024);

        assert_eq!(boundaries[2].index, 3);
        assert_eq!(boundaries[2].start, 20 * 1024 * 1024);
        assert_eq!(boundaries[2].end, 30 * 1024 * 1024);
    }

    #[test]
    fn boundary_calculations_with_remainder() {
        let session = UploadSession::new("alice", "up-1", "/path", 25 * 1024 * 1024);
        let chunk_size = 10 * 1024 * 1024;
        assert_eq!(session.expected_chunk_count(chunk_size), Some(3));

        let boundaries = session.chunk_boundaries(chunk_size).unwrap();
        assert_eq!(boundaries.len(), 3);

        assert_eq!(boundaries[2].index, 3);
        assert_eq!(boundaries[2].start, 20 * 1024 * 1024);
        assert_eq!(boundaries[2].end, 25 * 1024 * 1024);
        assert_eq!(boundaries[2].len(), 5 * 1024 * 1024);
    }

    #[test]
    fn zero_size_and_invalid_chunk_size() {
        let session = UploadSession::new("alice", "up-1", "/path", 0);
        assert_eq!(session.expected_chunk_count(1024), Some(1));
        assert_eq!(
            session.chunk_boundary(1, 1024),
            Some(ChunkBoundary::new(1, 0, 0))
        );
        assert!(session.chunk_boundary(1, 1024).unwrap().is_empty());

        assert_eq!(session.expected_chunk_count(0), None);
        assert_eq!(session.chunk_boundary(1, 0), None);
        assert_eq!(session.chunk_boundaries(0), None);

        assert_eq!(session.chunk_boundary(0, 1024), None);
        assert_eq!(session.chunk_boundary(2, 1024), None);
    }

    #[test]
    fn chunk_tracking_and_completion() {
        let session = UploadSession::new("alice", "up-1", "/path", 20 * 1024 * 1024);
        let chunk_size = 10 * 1024 * 1024;

        assert_eq!(session.uploaded_chunk_count(), 0);
        assert!(!session.is_chunk_uploaded(1));
        assert!(!session.is_complete(chunk_size));

        session.mark_chunk_uploaded(1);
        assert_eq!(session.uploaded_chunk_count(), 1);
        assert!(session.is_chunk_uploaded(1));
        assert!(!session.is_complete(chunk_size));

        session.mark_chunk_uploaded(2);
        assert_eq!(session.uploaded_chunk_count(), 2);
        assert!(session.is_chunk_uploaded(2));
        assert!(session.is_complete(chunk_size));

        let uploaded = session.uploaded_chunks();
        assert_eq!(uploaded.into_iter().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn cloned_session_shares_upload_progress() {
        let session = UploadSession::new("alice", "up-1", "/path", 30 * 1024 * 1024);
        let clone = session.clone();

        session.mark_chunk_uploaded(1);
        assert!(clone.is_chunk_uploaded(1));
        assert_eq!(clone.uploaded_chunk_count(), 1);

        clone.mark_chunk_uploaded(2);
        assert!(session.is_chunk_uploaded(2));
        assert_eq!(session.uploaded_chunk_count(), 2);
    }
}
