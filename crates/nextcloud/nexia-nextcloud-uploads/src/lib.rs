//! Nextcloud WebDAV Chunked Upload v2 API client.
//!
//! Provides [`UploadClient`] for uploading large files to Nextcloud using the
//! WebDAV Chunked Upload v2 protocol, and [`UploadSession`] for tracking active
//! upload progress and calculating chunk ranges.
//!
//! # Protocol Overview
//!
//! 1. **Initialize Upload Session**:
//!    - Sends `MKCOL /remote.php/dav/uploads/{user}/{upload_id}`
//!    - Required header: `Destination: {destination_webdav_url_or_path}`
//! 2. **Upload Chunks**:
//!    - Sends `PUT /remote.php/dav/uploads/{user}/{upload_id}/{chunk_number}` (e.g. `00001`, `00002`)
//!    - Required headers: `Destination: {destination_path}`, `OC-Total-Length: {total_file_size}`
//!    - Request body: chunk bytes (recommended 5 MiB to 5 GiB, last chunk may be smaller)
//! 3. **Assemble File**:
//!    - Sends `MOVE /remote.php/dav/uploads/{user}/{upload_id}/.file`
//!    - Required headers: `Destination: {destination_path}`, `OC-Total-Length: {total_file_size}`
//!    - Optional header: `X-OC-Mtime: {unix_timestamp}`
//! 4. **Abort Upload**:
//!    - Sends `DELETE /remote.php/dav/uploads/{user}/{upload_id}/` (cancels upload and cleans up temporary chunks)
//!
//! # Example
//!
//! ```no_run
//! use nexia_nextcloud_uploads::{UploadClient, UploadSession, DEFAULT_CHUNK_SIZE};
//! # use nexia_nextcloud_core::{Transport, AuthProvider};
//! # async fn run<T: Transport, A: AuthProvider>(transport: &T, auth: &A) -> Result<(), nexia_nextcloud_core::Error> {
//! let client = UploadClient::new(transport, auth);
//!
//! // 1. Start the upload session
//! let total_size = 25 * 1024 * 1024; // 25 MiB
//! let session = client.start("alice", "Documents/archive.zip", total_size).await?;
//!
//! // 2. Calculate chunk boundaries and upload each chunk
//! for boundary in session.chunk_boundaries(DEFAULT_CHUNK_SIZE).unwrap() {
//!     let chunk_bytes = vec![0u8; boundary.len() as usize];
//!     client.upload_chunk(&session, boundary.index, chunk_bytes).await?;
//! }
//!
//! // 3. Finish assembly
//! client.finish(&session, None).await?;
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
pub mod path;
mod session;

pub use client::{DESTINATION_HEADER, MTIME_HEADER, TOTAL_LENGTH_HEADER, UploadClient};
pub use path::{abort_path, assembly_path, chunk_path, upload_collection_path};
pub use session::{
    ChunkBoundary, DEFAULT_CHUNK_SIZE, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE, UploadSession,
};
