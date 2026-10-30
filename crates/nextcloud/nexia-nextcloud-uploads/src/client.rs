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

#[cfg(test)]
mod tests;

/// Header conveying total file size during chunked upload and assembly.
pub const TOTAL_LENGTH_HEADER: &str = "OC-Total-Length";

/// Header conveying target modification time (in unix seconds) during chunk assembly.
pub const MTIME_HEADER: &str = "X-OC-Mtime";

static UPLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generates a unique upload session token from system timestamp and an atomic counter.
fn generate_upload_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let count = UPLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("chunked-upload-{now}-{count}")
}

/// Client for Nextcloud's WebDAV Chunked Upload v2 protocol.
///
/// Manages creating upload directories, streaming individual chunks, assembling the
/// uploaded chunks into the destination file, or aborting the upload.
///
/// Generic over the [`Transport`] and [`AuthProvider`], held by reference so that
/// connection pools and credentials can be shared across tasks.
#[derive(Clone, Copy, Debug)]
pub struct UploadClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
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
    pub async fn start(
        &self,
        user_id: &str,
        destination_path: &str,
        total_size: u64,
    ) -> Result<UploadSession, Error> {
        let upload_id = generate_upload_id();
        self.start_with_id(user_id, destination_path, total_size, &upload_id)
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
        upload_id: &str,
    ) -> Result<UploadSession, Error> {
        tracing::info!(
            user_id,
            destination_path,
            total_size,
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

        Ok(UploadSession::new(
            user_id,
            upload_id,
            destination,
            total_size,
        ))
    }

    /// Uploads an individual chunk into the upload collection.
    ///
    /// Sends `PUT /remote.php/dav/uploads/{user_id}/{upload_id}/{chunk_number}` with:
    /// - `Destination: {destination_path}`
    /// - `OC-Total-Length: {total_size}`
    /// - payload containing `chunk_data`
    ///
    /// Automatically marks the chunk index as uploaded in the session upon success.
    pub async fn upload_chunk(
        &self,
        session: &UploadSession,
        chunk_index: u32,
        chunk_data: Vec<u8>,
    ) -> Result<(), Error> {
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

    /// Finalizes the upload by moving `.file` to the destination path.
    ///
    /// Sends `MOVE /remote.php/dav/uploads/{user_id}/{upload_id}/.file` with:
    /// - `Destination: {destination_path}`
    /// - `OC-Total-Length: {total_size}`
    /// - optionally `X-OC-Mtime: {mtime}`
    pub async fn finish(&self, session: &UploadSession, mtime: Option<u64>) -> Result<(), Error> {
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
