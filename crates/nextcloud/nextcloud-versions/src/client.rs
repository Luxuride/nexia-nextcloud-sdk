//! Asynchronous client for Nextcloud WebDAV file versions.

use nexia_nextcloud_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, StatusCode,
    StatusError, Transport,
};
use nextcloud_dav::{DEPTH_HEADER, DESTINATION_HEADER, DavProperty, Depth, PropfindBody};

use crate::decode::decode_multistatus;
use crate::validation::{validate_revision, validate_user};
use crate::version::FileVersion;

#[cfg(test)]
mod tests;

/// Content type for WebDAV PROPFIND requests.
const XML_CONTENT_TYPE: &str = "application/xml; charset=utf-8";

/// Client for Nextcloud's WebDAV file versions endpoint.
///
/// Interacts with `/remote.php/dav/versions/{user}/...` to list, download, and restore
/// previous versions of a file.
///
/// Generic over the [`Transport`] and [`AuthProvider`], both held by reference so that
/// connection pools and credentials can be shared across tasks.
#[derive(Clone, Copy, Debug)]
pub struct VersionsClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> VersionsClient<'a, T, A> {
    /// Binds a versions client to a transport and credential provider.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// The transport this client sends requests through.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// The credential provider applied to every call.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Lists all available revisions of a file.
    ///
    /// Sends a `PROPFIND` request to `/remote.php/dav/versions/{user_id}/versions/{file_id}`
    /// requesting revision metadata (`getlastmodified`, `getcontentlength`, and `getetag`).
    pub async fn list(&self, user_id: &str, file_id: u64) -> Result<Vec<FileVersion>, Error> {
        tracing::debug!(user_id, file_id, "Listing file versions");
        let user = validate_user(user_id)?;
        let path = ApiPath::dav(format!("versions/{user}/versions/{file_id}"))?;

        let body = PropfindBody::new([
            DavProperty::LastModified,
            DavProperty::ContentLength,
            DavProperty::Etag,
            DavProperty::ResourceType,
        ]);

        let mut headers = self.authenticated_headers();
        headers.set(DEPTH_HEADER, Depth::One.as_header_value());
        headers.set("Content-Type", XML_CONTENT_TYPE);

        let request = Request::new(HttpMethod::Propfind, path.clone())
            .with_headers(headers)
            .body_bytes(body.to_xml().into_bytes());

        let response = self.transport.execute(request).await?;

        if response.status() != StatusCode::new(207) {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Propfind.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        let body_str = std::str::from_utf8(response.body())
            .map_err(|e| Error::Decode(DecodeError::new("FileVersion", e)))?;

        decode_multistatus(body_str, file_id).map_err(Error::Decode)
    }

    /// Downloads the content of a specific revision.
    ///
    /// Sends a `GET` request to `/remote.php/dav/versions/{user_id}/versions/{file_id}/{revision_id}`.
    pub async fn get(
        &self,
        user_id: &str,
        file_id: u64,
        revision_id: &str,
    ) -> Result<Vec<u8>, Error> {
        tracing::debug!(
            user_id,
            file_id,
            revision_id,
            "Downloading file version content"
        );
        let user = validate_user(user_id)?;
        let revision = validate_revision(revision_id)?;
        let path = ApiPath::dav(format!("versions/{user}/versions/{file_id}/{revision}"))?;

        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Get, path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;

        if !response.status().is_success() {
            tracing::warn!(
                user_id,
                file_id,
                revision_id,
                status = %response.status(),
                "Failed to download file version"
            );
            return Err(Error::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        Ok(response.into_body())
    }

    /// Restores a past revision to be the current file content.
    ///
    /// Sends a `MOVE` request from `/remote.php/dav/versions/{user_id}/versions/{file_id}/{revision_id}`
    /// to `/remote.php/dav/versions/{user_id}/restore/{file_id}`.
    pub async fn restore(
        &self,
        user_id: &str,
        file_id: u64,
        revision_id: &str,
    ) -> Result<(), Error> {
        tracing::info!(user_id, file_id, revision_id, "Restoring file version");
        let user = validate_user(user_id)?;
        let revision = validate_revision(revision_id)?;

        let source = ApiPath::dav(format!("versions/{user}/versions/{file_id}/{revision}"))?;
        let destination = ApiPath::dav(format!("versions/{user}/restore/{file_id}"))?;

        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, destination.as_str());

        let request = Request::new(HttpMethod::Move, source.clone()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Move.as_str(),
                source,
                response.status(),
                response.body(),
            )));
        }

        Ok(())
    }

    fn authenticated_headers(&self) -> Headers {
        let mut headers = Headers::new();
        self.auth.authenticate(&mut headers);
        headers
    }
}
