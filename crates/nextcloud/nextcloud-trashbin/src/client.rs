//! WebDAV client for Nextcloud trashbin management.

use std::fmt;

use nextcloud_dav::{DEPTH_HEADER, DESTINATION_HEADER, OVERWRITE_HEADER, Overwrite};
use nextcloud_sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, Response, StatusCode,
    StatusError, Transport,
};

use crate::item::TrashItem;
use crate::path::{restore_destination_path, trash_item_path, trash_root_path};
use crate::propfind::{build_propfind_body, decode_multistatus};

/// Content-Type header value for XML request payloads.
const XML_CONTENT_TYPE: &str = "application/xml; charset=utf-8";

/// WebDAV client for interacting with Nextcloud's trashbin endpoints.
///
/// Executes calls through any [`Transport`] and applies credentials via an [`AuthProvider`].
/// Both are held by reference, allowing the client to be freely cloned or borrowed across
/// tasks without runtime dispatch or connection pooling overhead.
///
/// Supported operations:
/// - [`list`](Self::list) — inspects deleted items
/// - [`restore`](Self::restore) — restores an item to its original or specified path
/// - [`delete`](Self::delete) — permanently deletes a specific item
/// - [`empty`](Self::empty) — permanently deletes all items in the trashbin
#[derive(Clone, Copy, Debug)]
pub struct TrashClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> TrashClient<'a, T, A> {
    /// Binds a trashbin client to a transport and credential provider.
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

    /// Lists all items currently in `user_id`'s trashbin.
    ///
    /// Sends a `PROPFIND` request with `Depth: 1` to `/remote.php/dav/trashbin/{user_id}/trash/`.
    ///
    /// The response must be `207 Multi-Status`; any other status becomes [`Error::Status`].
    pub async fn list(&self, user_id: &str) -> Result<Vec<TrashItem>, Error> {
        tracing::debug!(user_id, "Listing trashbin items");
        let path = trash_root_path(user_id)?;
        let mut headers = self.authenticated_headers();
        headers.set(DEPTH_HEADER, "1");
        headers.set("Content-Type", XML_CONTENT_TYPE);

        let body = build_propfind_body();
        let request = Request::new(HttpMethod::Propfind, path.clone())
            .with_headers(headers)
            .body_bytes(body.into_bytes());

        let response = self.transport.execute(request).await?;

        if response.status() != StatusCode::new(207) {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Propfind.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        let text = std::str::from_utf8(response.body())
            .map_err(|error| Error::Decode(DecodeError::new(crate::propfind::TARGET, error)))?;
        decode_multistatus(user_id, text).map_err(Error::Decode)
    }

    /// Restores a deleted item from the trashbin.
    ///
    /// Sends a `MOVE` request from `/remote.php/dav/trashbin/{user_id}/trash/{item_id}`
    /// to `/remote.php/dav/trashbin/{user_id}/restore/{restore_path}` (or the restore
    /// folder if `restore_path` is empty).
    ///
    /// Any non-`2xx` status returns [`Error::Status`].
    pub async fn restore(
        &self,
        user_id: &str,
        item_id: &str,
        restore_path: &str,
    ) -> Result<(), Error> {
        tracing::info!(user_id, item_id, restore_path, "Restoring trashbin item");
        let source_path = trash_item_path(user_id, item_id)?;
        let destination_path = restore_destination_path(user_id, restore_path)?;

        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, destination_path.as_str());
        headers.set(OVERWRITE_HEADER, Overwrite::Allow.as_header_value());

        let request = Request::new(HttpMethod::Move, source_path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Move, &source_path, &response)
    }

    /// Permanently deletes a single item from the trashbin.
    ///
    /// Sends a `DELETE` request to `/remote.php/dav/trashbin/{user_id}/trash/{item_id}`.
    ///
    /// Any non-`2xx` status returns [`Error::Status`].
    pub async fn delete(&self, user_id: &str, item_id: &str) -> Result<(), Error> {
        tracing::info!(user_id, item_id, "Permanently deleting trashbin item");
        let path = trash_item_path(user_id, item_id)?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, path.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Delete, &path, &response)
    }

    /// Permanently deletes all items in the user's trashbin.
    ///
    /// Sends a `DELETE` request to `/remote.php/dav/trashbin/{user_id}/trash/`.
    ///
    /// Any non-`2xx` status returns [`Error::Status`].
    pub async fn empty(&self, user_id: &str) -> Result<(), Error> {
        tracing::warn!(user_id, "Emptying trashbin");
        let path = trash_root_path(user_id)?;
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
            "Trashbin operation failed"
        );
        return Err(Error::Status(StatusError::new(
            method.as_str(),
            path.clone(),
            response.status(),
            response.body(),
        )));
    }
    tracing::trace!(method = ?method, path = %path.as_str(), "Trashbin operation succeeded");
    Ok(())
}

impl<T: Transport, A: AuthProvider> fmt::Display for TrashClient<'_, T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TrashClient")
    }
}
