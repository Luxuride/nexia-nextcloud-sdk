//! Asynchronous client for Nextcloud's WebDAV Chunked Upload v2 API.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use nexia_nextcloud_core::{
    ApiPath, AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusError, Transport,
};
pub use nexia_nextcloud_dav::DESTINATION_HEADER;

use crate::path::{
    abort_path, assembly_path, chunk_path, destination_header_value, upload_collection_path,
};
use crate::session::UploadSession;

/// A session that cannot be uploaded or assembled as it stands.
#[derive(Debug)]
struct UploadProtocolError(String);

impl fmt::Display for UploadProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UploadProtocolError {}

#[cfg(test)]
mod tests;

/// Header conveying total file size during chunked upload and assembly.
pub const TOTAL_LENGTH_HEADER: &str = "OC-Total-Length";

/// Header conveying target modification time (in unix seconds) during chunk assembly.
pub const MTIME_HEADER: &str = "X-OC-Mtime";

/// Number of random bytes in an upload id.
const UPLOAD_ID_BYTES: usize = 16;

/// Fallback counter used only when the operating system's random source is unavailable.
static UPLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generates an unpredictable upload session id.
///
/// The id names a server-side collection that anyone who can guess it can write into, so it is
/// derived from the operating system's random source rather than from the clock. The old
/// `<millis>-<counter>` form was guessable and collided across processes.
fn generate_upload_id() -> String {
    let mut id = String::with_capacity("chunked-upload-".len() + UPLOAD_ID_BYTES * 2);
    id.push_str("chunked-upload-");

    let mut bytes = [0u8; UPLOAD_ID_BYTES];
    match getrandom::fill(&mut bytes) {
        Ok(()) => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for byte in bytes {
                id.push(HEX[usize::from(byte >> 4)] as char);
                id.push(HEX[usize::from(byte & 0x0f)] as char);
            }
        }
        Err(error) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let count = UPLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                %error,
                "no random source for the upload id; falling back to the clock"
            );
            id = format!("chunked-upload-{now}-{count}");
        }
    }

    id
}

/// Client for Nextcloud's WebDAV Chunked Upload v2 protocol.
///
/// Manages creating upload directories, streaming individual chunks, assembling the
/// uploaded chunks into the destination file, or aborting the upload.
///
/// Generic over the [`Transport`] and [`AuthProvider`], held by reference so that
/// connection pools and credentials can be shared across tasks.
#[derive(Clone, Copy)]
pub struct UploadClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<T: Transport, A: AuthProvider> fmt::Debug for UploadClient<'_, T, A> {
    /// The client holds no state worth logging, and the credential source may.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UploadClient")
    }
}

impl<'a, T: Transport, A: AuthProvider> UploadClient<'a, T, A> {
    /// Binds an upload client to a transport and credential provider.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// The transport this client sends requests through.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// The credential provider applied to every request.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Initializes a new chunked upload session by sending an `MKCOL` request with an auto-generated upload ID.
    ///
    /// Sends `MKCOL /remote.php/dav/uploads/{user_id}/{upload_id}` with the `Destination` header set.
    /// The session remembers `chunk_size`, which is what lets [`Self::upload_chunk`] validate a chunk
    /// and [`Self::finish`] refuse to assemble an incomplete upload.
    pub async fn start(
        &self,
        user_id: &str,
        destination_path: &str,
        total_size: u64,
        chunk_size: u64,
    ) -> Result<UploadSession, Error> {
        let upload_id = generate_upload_id();
        self.start_with_id(
            user_id,
            destination_path,
            total_size,
            chunk_size,
            &upload_id,
        )
        .await
    }

    /// Initializes a new chunked upload session using an explicit `upload_id`.
    ///
    /// Sends `MKCOL /remote.php/dav/uploads/{user_id}/{upload_id}` with the `Destination` header set.
    pub async fn start_with_id(
        &self,
        user_id: &str,
        destination_path: &str,
        total_size: u64,
        chunk_size: u64,
        upload_id: &str,
    ) -> Result<UploadSession, Error> {
        validate_chunk_size(chunk_size)?;
        tracing::info!(
            user_id,
            destination_path,
            total_size,
            chunk_size,
            upload_id,
            "Starting chunked upload session"
        );
        let destination = destination_header_value(user_id, destination_path)?;
        let path = upload_collection_path(user_id, upload_id)?;

        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, &destination);

        let request = Request::new(HttpMethod::Mkcol, path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Mkcol, &path, &response)?;

        Ok(
            UploadSession::new(user_id, upload_id, destination, total_size)
                .with_chunk_size(chunk_size),
        )
    }

    /// Uploads an individual chunk into the upload collection.
    ///
    /// Sends `PUT /remote.php/dav/uploads/{user_id}/{upload_id}/{chunk_number}` with:
    /// - `Destination: {destination_path}`
    /// - `OC-Total-Length: {total_size}`
    /// - payload containing `chunk_data`
    ///
    /// The chunk must carry exactly the bytes its boundary in the session prescribes: a short chunk
    /// assembled with the others produces a file whose every later byte is shifted, and the server
    /// reports the assembly as successful. Automatically marks the chunk index as uploaded upon
    /// success.
    pub async fn upload_chunk(
        &self,
        session: &UploadSession,
        chunk_index: u32,
        chunk_data: Vec<u8>,
    ) -> Result<(), Error> {
        let chunk_size = session.chunk_size().ok_or_else(|| {
            Error::invalid_response(UploadProtocolError(format!(
                "upload '{}' has no chunk size; build the session with `UploadSession::with_chunk_size`",
                session.upload_id()
            )))
        })?;
        let boundary = session
            .chunk_boundary(chunk_index, chunk_size)
            .ok_or_else(|| {
                let expected = session
                    .expected_chunk_count(chunk_size)
                    .map_or_else(|| "an unknown number".to_string(), |count| count.to_string());
                Error::invalid_response(UploadProtocolError(format!(
                    "chunk {chunk_index} is out of range for upload '{}', which has {expected} chunks",
                    session.upload_id()
                )))
            })?;
        if chunk_data.len() as u64 != boundary.len() {
            return Err(Error::invalid_response(UploadProtocolError(format!(
                "chunk {chunk_index} of upload '{}' must carry {} bytes, got {}",
                session.upload_id(),
                boundary.len(),
                chunk_data.len()
            ))));
        }

        let path = chunk_path(session.user_id(), session.upload_id(), chunk_index)?;
        tracing::debug!(
            upload_id = session.upload_id(),
            chunk_index,
            bytes = chunk_data.len(),
            "Uploading file chunk"
        );

        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, session.destination_path());
        headers.set(TOTAL_LENGTH_HEADER, session.total_size().to_string());

        let request = Request::new(HttpMethod::Put, path.clone())
            .with_headers(headers)
            .body_bytes(chunk_data);

        let response = self.transport.execute(request).await?;
        ensure_success(HttpMethod::Put, &path, &response)?;

        session.mark_chunk_uploaded(chunk_index);
        Ok(())
    }

    /// Finalizes the upload after verifying that every expected chunk arrived.
    ///
    /// Assembly moves whatever chunks the server holds into the destination, so finalizing an
    /// incomplete session stores a truncated file under the real name and its missing tail can no
    /// longer be uploaded. The session therefore has to know its chunk size; sessions created by
    /// [`Self::start`] always do.
    ///
    /// # Errors
    /// Returns [`Error::Decode`] when the session has no chunk size or is incomplete, plus
    /// everything the assembly request can return.
    pub async fn finish(&self, session: &UploadSession, mtime: Option<u64>) -> Result<(), Error> {
        let chunk_size = session.chunk_size().ok_or_else(|| {
            Error::invalid_response(UploadProtocolError(format!(
                "upload '{}' has no chunk size, so its completeness cannot be verified",
                session.upload_id()
            )))
        })?;
        let expected = session.expected_chunk_count(chunk_size).ok_or_else(|| {
            Error::invalid_response(UploadProtocolError(format!(
                "upload '{}' has an unsupported chunk size of {chunk_size} bytes",
                session.upload_id()
            )))
        })?;
        if !session.is_complete(chunk_size) {
            return Err(Error::invalid_response(UploadProtocolError(format!(
                "upload '{}' is incomplete: {} of {expected} chunks arrived",
                session.upload_id(),
                session.uploaded_chunk_count()
            ))));
        }

        let path = assembly_path(session.user_id(), session.upload_id())?;
        tracing::info!(
            upload_id = session.upload_id(),
            destination = session.destination_path(),
            "Finalizing chunked upload assembly"
        );

        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, session.destination_path());
        headers.set(TOTAL_LENGTH_HEADER, session.total_size().to_string());
        if let Some(mtime) = mtime {
            headers.set(MTIME_HEADER, mtime.to_string());
        }

        let request = Request::new(HttpMethod::Move, path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Move, &path, &response)
    }

    /// Cancels and removes the temporary upload collection.
    ///
    /// Sends `DELETE /remote.php/dav/uploads/{user_id}/{upload_id}/`.
    pub async fn abort(&self, session: &UploadSession) -> Result<(), Error> {
        let path = abort_path(session.user_id(), session.upload_id())?;
        tracing::warn!(
            upload_id = session.upload_id(),
            "Aborting chunked upload session"
        );

        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Delete, &path, &response)
    }

    fn authenticated_headers(&self) -> Headers {
        let mut headers = Headers::new();
        self.auth.authenticate(&mut headers);
        headers
    }
}

/// Rejects a chunk size the protocol cannot express and warns about one outside the
/// recommendation.
///
/// A zero chunk size would divide by zero when computing boundaries, so it is an error. The
/// documented 5 MiB to 5 GiB range is a recommendation Nextcloud also enforces on its side, not a
/// rule this client has to break the upload over, so leaving it only warns.
fn validate_chunk_size(chunk_size: u64) -> Result<(), Error> {
    if chunk_size == 0 {
        return Err(Error::invalid_response(UploadProtocolError(
            "the chunk size must not be zero".to_string(),
        )));
    }
    if !UploadSession::is_supported_chunk_size(chunk_size) {
        tracing::warn!(
            chunk_size,
            "chunk size is outside the recommended 5 MiB to 5 GiB range"
        );
    }
    Ok(())
}

fn ensure_success(method: HttpMethod, path: &ApiPath, response: &Response) -> Result<(), Error> {
    if !response.status().is_success() {
        tracing::error!(
            method = ?method,
            path = %path.as_str(),
            status = %response.status(),
            "Chunked upload operation failed"
        );
        return Err(Error::Status(StatusError::new(
            method.as_str(),
            path.clone(),
            response.status(),
            response.body(),
        )));
    }
    tracing::trace!(method = ?method, path = %path.as_str(), "Chunked upload request succeeded");
    Ok(())
}

impl<T: Transport, A: AuthProvider> fmt::Display for UploadClient<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UploadClient")
    }
}
