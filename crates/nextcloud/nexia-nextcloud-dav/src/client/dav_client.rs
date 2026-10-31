//! Request plumbing for WebDAV calls.

use nexia_nextcloud_core::{
    AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, Response, StatusCode,
    StatusError, Transport,
};

use super::multistatus;
use crate::method::{DEPTH_HEADER, DESTINATION_HEADER, Depth, OVERWRITE_HEADER, Overwrite};
use crate::path::DavPath;
use crate::propfind::PropfindBody;
use crate::resource::DavResource;

/// MIME type every WebDAV body in this crate is sent with.
const XML_CONTENT_TYPE: &str = "application/xml; charset=utf-8";

/// WebDAV client generic over the transport and the credential source.
///
/// Both are held by reference, so a client can be borrowed into several
/// spawned tasks and share one connection pool and one token.
#[derive(Clone, Copy)]
pub struct DavClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> DavClient<'a, T, A> {
    /// Binds a client to a transport and a credential source.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// Transport this client sends through.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// Credential source applied to every request.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Asks `path` for the properties in `body`, `depth` levels deep.
    ///
    /// The answer must be `207 Multi-Status`; any other status becomes
    /// [`Error::Status`] with the response body retained for diagnosis. A
    /// body that is not a multistatus becomes [`Error::Decode`].
    pub async fn propfind(
        &self,
        path: &DavPath,
        depth: Depth,
        body: &PropfindBody,
    ) -> Result<Vec<DavResource>, Error> {
        tracing::debug!(path = %path.as_str(), ?depth, "Executing WebDAV PROPFIND");
        let request = self.build_propfind(path, depth, body);
        let response = self.transport.execute(request).await?;

        if response.status() != StatusCode::new(207) {
            tracing::warn!(
                path = %path.as_str(),
                status = %response.status(),
                "WebDAV PROPFIND expected 207 Multi-Status"
            );
            return Err(Error::Status(StatusError::new(
                HttpMethod::Propfind.as_str(),
                path.to_api_path(),
                response.status(),
                response.body(),
            )));
        }

        let text = std::str::from_utf8(response.body())
            .map_err(|error| Error::Decode(DecodeError::new(multistatus::TARGET, error)))?;
        multistatus::decode(text).map_err(Error::Decode)
    }

    /// Downloads the content of the resource at `path`.
    ///
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn get(&self, path: &DavPath) -> Result<Vec<u8>, Error> {
        tracing::debug!(path = %path.as_str(), "Executing WebDAV GET");
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Get, path.to_api_path()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Get, path, &response)?;
        Ok(response.into_body())
    }

    /// Fetches a byte range from `path` using the HTTP `Range` header.
    ///
    /// `start` and `end` are inclusive offsets. Any non-`2xx` status returns
    /// [`Error::Status`] with the response body retained for diagnosis.
    ///
    /// A server is allowed to ignore `Range` and answer `200` with the whole
    /// representation; that answer is reduced to the requested window here,
    /// because returning the first bytes of the file as if they were the range
    /// silently corrupts every caller that caches the result.
    pub async fn get_range(&self, path: &DavPath, start: u64, end: u64) -> Result<Vec<u8>, Error> {
        if end < start {
            return Err(Error::invalid_response(RangeMismatch(format!(
                "range {start}-{end} is inverted"
            ))));
        }
        tracing::debug!(path = %path.as_str(), start, end, "Executing WebDAV GET with Range");
        let mut headers = self.authenticated_headers();
        headers.set("Range", format!("bytes={start}-{end}"));
        let request = Request::new(HttpMethod::Get, path.to_api_path()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Get, path, &response)?;
        let partial = response.status().as_u16() == 206;
        let server_start = ranged_start(response.headers());
        let body = response.into_body();
        if !partial {
            return Ok(window(&body, start, end));
        }
        match server_start {
            Some(server_start) if server_start > start => {
                Err(Error::invalid_response(RangeMismatch(format!(
                    "server answered the range from {server_start} while {start} was requested"
                ))))
            }
            Some(server_start) => Ok(window(&body, start - server_start, end - server_start)),
            None => Ok(body),
        }
    }

    /// Uploads `body` to `path`, optionally specifying the `Content-Type` header.
    ///
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn put(
        &self,
        path: &DavPath,
        body: impl Into<Vec<u8>>,
        content_type: Option<&str>,
    ) -> Result<(), Error> {
        tracing::debug!(path = %path.as_str(), "Executing WebDAV PUT");
        let mut headers = self.authenticated_headers();
        if let Some(content_type) = content_type {
            headers.set("Content-Type", content_type);
        }
        let request = Request::new(HttpMethod::Put, path.to_api_path())
            .with_headers(headers)
            .body_bytes(body);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Put, path, &response)
    }

    /// Creates a collection (folder) at `path`.
    ///
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn mkcol(&self, path: &DavPath) -> Result<(), Error> {
        tracing::debug!(path = %path.as_str(), "Executing WebDAV MKCOL");
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Mkcol, path.to_api_path()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Mkcol, path, &response)
    }

    /// Deletes the resource or collection at `path`.
    ///
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn delete(&self, path: &DavPath) -> Result<(), Error> {
        tracing::debug!(path = %path.as_str(), "Executing WebDAV DELETE");
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, path.to_api_path()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(HttpMethod::Delete, path, &response)
    }

    /// Copies a resource from `source` to `destination`.
    ///
    /// When `overwrite` is `true`, an existing destination is replaced;
    /// otherwise the call fails with a status error if the destination exists.
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn copy(
        &self,
        source: &DavPath,
        destination: &DavPath,
        overwrite: bool,
    ) -> Result<(), Error> {
        self.transfer(HttpMethod::Copy, source, destination, overwrite)
            .await
    }

    /// Moves a resource from `source` to `destination`.
    ///
    /// When `overwrite` is `true`, an existing destination is replaced;
    /// otherwise the call fails with a status error if the destination exists.
    /// Any non-`2xx` status returns [`Error::Status`] with the response body
    /// retained for diagnosis.
    pub async fn move_(
        &self,
        source: &DavPath,
        destination: &DavPath,
        overwrite: bool,
    ) -> Result<(), Error> {
        self.transfer(HttpMethod::Move, source, destination, overwrite)
            .await
    }

    fn authenticated_headers(&self) -> Headers {
        // Credentials go on first: the headers set afterwards can then never
        // displace an `Authorization` the provider applied.
        let mut headers = Headers::new();
        self.auth.authenticate(&mut headers);
        headers
    }

    async fn transfer(
        &self,
        method: HttpMethod,
        source: &DavPath,
        destination: &DavPath,
        overwrite: bool,
    ) -> Result<(), Error> {
        tracing::debug!(
            method = ?method,
            source = %source.as_str(),
            destination = %destination.as_str(),
            overwrite,
            "Executing WebDAV transfer"
        );
        let mut headers = self.authenticated_headers();
        headers.set(DESTINATION_HEADER, destination.to_api_path().as_str());
        headers.set(
            OVERWRITE_HEADER,
            Overwrite::from(overwrite).as_header_value(),
        );
        let request = Request::new(method, source.to_api_path()).with_headers(headers);
        let response = self.transport.execute(request).await?;

        ensure_success(method, source, &response)
    }

    fn build_propfind(&self, path: &DavPath, depth: Depth, body: &PropfindBody) -> Request {
        let mut headers = self.authenticated_headers();
        headers.set(DEPTH_HEADER, depth.as_header_value());
        headers.set("Content-Type", XML_CONTENT_TYPE);

        Request::new(HttpMethod::Propfind, path.to_api_path())
            .with_headers(headers)
            .body_bytes(body.to_xml().into_bytes())
    }
}

/// A `Range` request whose answer cannot be reconciled with what was asked.
#[derive(Debug)]
struct RangeMismatch(String);

impl std::fmt::Display for RangeMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RangeMismatch {}

/// Inclusive `start..=end` window of `body`, clamped to its length.
fn window(body: &[u8], start: u64, end: u64) -> Vec<u8> {
    let len = body.len() as u64;
    let from = usize::try_from(start.min(len)).unwrap_or(body.len());
    let to = usize::try_from(end.saturating_add(1).min(len)).unwrap_or(body.len());
    if from >= to {
        return Vec::new();
    }
    body[from..to].to_vec()
}

/// Start offset of a `Content-Range: bytes <start>-<end>/<size>` header.
fn ranged_start(headers: &Headers) -> Option<u64> {
    let value = headers.get("content-range")?.trim();
    let (unit, range) = value.split_once(' ')?;
    if !unit.eq_ignore_ascii_case("bytes") {
        return None;
    }
    let (start, remainder) = range.split_once('-')?;
    remainder.split_once('/')?;
    start.parse().ok()
}

fn ensure_success(method: HttpMethod, path: &DavPath, response: &Response) -> Result<(), Error> {
    if !response.status().is_success() {
        tracing::warn!(
            method = ?method,
            path = %path.as_str(),
            status = %response.status(),
            "WebDAV request received non-success status"
        );
        return Err(Error::Status(StatusError::new(
            method.as_str(),
            path.to_api_path(),
            response.status(),
            response.body(),
        )));
    }
    tracing::trace!(method = ?method, path = %path.as_str(), "WebDAV request succeeded");
    Ok(())
}

impl<T: Transport, A: AuthProvider> std::fmt::Debug for DavClient<'_, T, A> {
    /// A client holds no state worth logging, and the credential source may.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DavClient")
    }
}

impl<T: Transport, A: AuthProvider> std::fmt::Display for DavClient<'_, T, A> {
    /// A client carries no identifying state worth logging.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DavClient")
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Mutex;
    use std::task::Poll;

    use nexia_nextcloud_core::{
        AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
    };

    use super::{DavClient, XML_CONTENT_TYPE};
    use crate::fixtures::TWO_ENTRIES;
    use crate::method::{DEPTH_HEADER, DESTINATION_HEADER, Depth, OVERWRITE_HEADER};
    use crate::path::DavPath;
    use crate::propfind::PropfindBody;
    use crate::resource::DavResource;

    /// Answers every request with one canned response and records what it was
    /// asked, so header and body construction are assertable without a server.
    struct FakeTransport {
        status: StatusCode,
        body: Vec<u8>,
        seen: Mutex<Vec<Request>>,
    }

    impl FakeTransport {
        fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
            Self {
                status: StatusCode::new(status),
                body: body.into(),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn last_request(&self) -> Request {
            let seen = self.seen.lock().expect("test mutex is not poisoned");
            seen.last().cloned().expect("a request was sent")
        }
    }

    impl Transport for FakeTransport {
        async fn execute(&self, request: Request) -> Result<Response, Error> {
            self.seen
                .lock()
                .expect("test mutex is not poisoned")
                .push(request);
            Ok(Response::new(
                self.status,
                Headers::new(),
                self.body.clone(),
            ))
        }
    }

    struct FakeAuth;

    impl AuthProvider for FakeAuth {
        fn authenticate(&self, headers: &mut Headers) {
            headers.set("Authorization", "Basic dGVzdDp0ZXN0");
        }
    }

    fn run<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
        }
    }

    fn propfind(status: u16, body: &str) -> (Result<Vec<DavResource>, Error>, FakeTransport) {
        let transport = FakeTransport::new(status, body);
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "Photos").expect("path is valid");
        let result = run(client.propfind(&path, Depth::One, &PropfindBody::all()));
        (result, transport)
    }

    #[test]
    fn propfind_sets_depth_content_type_and_auth() {
        let (result, transport) = propfind(207, TWO_ENTRIES);
        assert!(result.is_ok());

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Propfind);
        assert_eq!(
            request.path().as_str(),
            "/remote.php/dav/files/alice/Photos"
        );
        assert_eq!(request.headers().get(DEPTH_HEADER), Some("1"));
        assert_eq!(
            request.headers().get("Content-Type"),
            Some(XML_CONTENT_TYPE)
        );
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Basic dGVzdDp0ZXN0")
        );
    }

    #[test]
    fn propfind_sends_the_rendered_body() {
        let (_, transport) = propfind(207, TWO_ENTRIES);
        let request = transport.last_request();
        let body = std::str::from_utf8(request.body().expect("body")).expect("utf-8");
        assert!(body.contains("<d:propfind"));
        assert!(body.contains(r#"xmlns:d="DAV:""#));
        assert!(body.contains("<d:getetag/>"));
    }

    #[test]
    fn depth_token_reaches_the_header() {
        for (depth, expected) in [
            (Depth::Zero, "0"),
            (Depth::One, "1"),
            (Depth::Infinity, "infinity"),
        ] {
            let transport = FakeTransport::new(207, TWO_ENTRIES);
            let auth = FakeAuth;
            let client = DavClient::new(&transport, &auth);
            let path = DavPath::root("alice").expect("path is valid");
            let _ = run(client.propfind(&path, depth, &PropfindBody::all()));
            assert_eq!(
                transport.last_request().headers().get(DEPTH_HEADER),
                Some(expected)
            );
        }
    }

    #[test]
    fn successful_propfind_decodes_the_resources() {
        let (result, _) = propfind(207, TWO_ENTRIES);
        let resources = result.expect("decode succeeds");
        assert_eq!(resources.len(), 2);
        assert!(resources[0].is_collection);
        assert!(!resources[1].is_collection);
    }

    #[test]
    fn non_multistatus_status_becomes_a_status_error() {
        let (result, _) = propfind(404, "<d:error/>");
        let error = result.expect_err("404 is not a multistatus");
        assert_eq!(error.status_code(), Some(404));
        assert!(error.is_not_found());
    }

    #[test]
    fn unreadable_body_becomes_a_decode_error() {
        let (result, _) = propfind(207, "<html>not dav</html>");
        let error = result.expect_err("body is not a multistatus");
        let decode = error.as_decode().expect("decode stage");
        assert_eq!(decode.target(), "DavResource");
    }

    #[test]
    fn invalid_utf8_body_becomes_a_decode_error() {
        let transport = FakeTransport::new(207, vec![0xff, 0xfe]);
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::root("alice").expect("path is valid");
        let error = run(client.propfind(&path, Depth::Zero, &PropfindBody::all()))
            .expect_err("invalid utf-8");
        assert_eq!(error.stage().as_str(), "decode");
    }

    #[test]
    fn get_downloads_content_and_sets_auth() {
        let transport = FakeTransport::new(200, b"file payload");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "document.txt").expect("path is valid");

        let body = run(client.get(&path)).expect("get succeeds");
        assert_eq!(body, b"file payload");

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Get);
        assert_eq!(
            request.path().as_str(),
            "/remote.php/dav/files/alice/document.txt"
        );
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Basic dGVzdDp0ZXN0")
        );
    }

    #[test]
    fn get_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(404, "not found");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "missing.txt").expect("path is valid");

        let error = run(client.get(&path)).expect_err("404 returns error");
        assert_eq!(error.status_code(), Some(404));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "GET");
        assert_eq!(
            status.path().as_str(),
            "/remote.php/dav/files/alice/missing.txt"
        );
    }

    #[test]
    fn put_uploads_body_and_sets_content_type() {
        let transport = FakeTransport::new(201, b"");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "notes.md").expect("path is valid");

        run(client.put(&path, b"# Hello".to_vec(), Some("text/markdown"))).expect("put succeeds");

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Put);
        assert_eq!(
            request.path().as_str(),
            "/remote.php/dav/files/alice/notes.md"
        );
        assert_eq!(request.headers().get("Content-Type"), Some("text/markdown"));
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Basic dGVzdDp0ZXN0")
        );
        assert_eq!(request.body(), Some(&b"# Hello"[..]));
    }

    #[test]
    fn put_omits_content_type_when_none() {
        let transport = FakeTransport::new(204, b"");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "raw.bin").expect("path is valid");

        run(client.put(&path, vec![0xde, 0xad, 0xbe, 0xef], None)).expect("put succeeds");

        let request = transport.last_request();
        assert_eq!(request.headers().get("Content-Type"), None);
        assert_eq!(request.body(), Some(&[0xde, 0xad, 0xbe, 0xef][..]));
    }

    #[test]
    fn put_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(409, "conflict");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "notes.md").expect("path is valid");

        let error = run(client.put(&path, b"test".to_vec(), None)).expect_err("409 returns error");
        assert_eq!(error.status_code(), Some(409));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "PUT");
    }

    #[test]
    fn mkcol_creates_collection() {
        let transport = FakeTransport::new(201, b"");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "NewFolder").expect("path is valid");

        run(client.mkcol(&path)).expect("mkcol succeeds");

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Mkcol);
        assert_eq!(
            request.path().as_str(),
            "/remote.php/dav/files/alice/NewFolder"
        );
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Basic dGVzdDp0ZXN0")
        );
        assert!(request.body().is_none());
    }

    #[test]
    fn mkcol_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(405, "not allowed");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "Folder").expect("path is valid");

        let error = run(client.mkcol(&path)).expect_err("405 returns error");
        assert_eq!(error.status_code(), Some(405));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "MKCOL");
    }

    #[test]
    fn delete_removes_resource() {
        let transport = FakeTransport::new(204, b"");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "old.txt").expect("path is valid");

        run(client.delete(&path)).expect("delete succeeds");

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Delete);
        assert_eq!(
            request.path().as_str(),
            "/remote.php/dav/files/alice/old.txt"
        );
        assert_eq!(
            request.headers().get("Authorization"),
            Some("Basic dGVzdDp0ZXN0")
        );
    }

    #[test]
    fn delete_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(404, "not found");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "old.txt").expect("path is valid");

        let error = run(client.delete(&path)).expect_err("404 returns error");
        assert_eq!(error.status_code(), Some(404));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "DELETE");
    }

    #[test]
    fn copy_sends_destination_and_overwrite_headers() {
        for (overwrite, expected_header) in [(true, "T"), (false, "F")] {
            let transport = FakeTransport::new(201, b"");
            let auth = FakeAuth;
            let client = DavClient::new(&transport, &auth);
            let source = DavPath::new("alice", "src.txt").expect("path is valid");
            let destination = DavPath::new("alice", "dst.txt").expect("path is valid");

            run(client.copy(&source, &destination, overwrite)).expect("copy succeeds");

            let request = transport.last_request();
            assert_eq!(request.method(), HttpMethod::Copy);
            assert_eq!(
                request.path().as_str(),
                "/remote.php/dav/files/alice/src.txt"
            );
            assert_eq!(
                request.headers().get(DESTINATION_HEADER),
                Some("/remote.php/dav/files/alice/dst.txt")
            );
            assert_eq!(
                request.headers().get(OVERWRITE_HEADER),
                Some(expected_header)
            );
            assert_eq!(
                request.headers().get("Authorization"),
                Some("Basic dGVzdDp0ZXN0")
            );
        }
    }

    #[test]
    fn copy_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(412, "precondition failed");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let source = DavPath::new("alice", "src.txt").expect("path is valid");
        let destination = DavPath::new("alice", "dst.txt").expect("path is valid");

        let error = run(client.copy(&source, &destination, false)).expect_err("412 returns error");
        assert_eq!(error.status_code(), Some(412));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "COPY");
        assert_eq!(
            status.path().as_str(),
            "/remote.php/dav/files/alice/src.txt"
        );
    }

    #[test]
    fn move_sends_destination_and_overwrite_headers() {
        for (overwrite, expected_header) in [(true, "T"), (false, "F")] {
            let transport = FakeTransport::new(201, b"");
            let auth = FakeAuth;
            let client = DavClient::new(&transport, &auth);
            let source = DavPath::new("alice", "src.txt").expect("path is valid");
            let destination = DavPath::new("alice", "dst.txt").expect("path is valid");

            run(client.move_(&source, &destination, overwrite)).expect("move succeeds");

            let request = transport.last_request();
            assert_eq!(request.method(), HttpMethod::Move);
            assert_eq!(
                request.path().as_str(),
                "/remote.php/dav/files/alice/src.txt"
            );
            assert_eq!(
                request.headers().get(DESTINATION_HEADER),
                Some("/remote.php/dav/files/alice/dst.txt")
            );
            assert_eq!(
                request.headers().get(OVERWRITE_HEADER),
                Some(expected_header)
            );
            assert_eq!(
                request.headers().get("Authorization"),
                Some("Basic dGVzdDp0ZXN0")
            );
        }
    }

    #[test]
    fn move_non_success_becomes_a_status_error() {
        let transport = FakeTransport::new(412, "precondition failed");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let source = DavPath::new("alice", "src.txt").expect("path is valid");
        let destination = DavPath::new("alice", "dst.txt").expect("path is valid");

        let error = run(client.move_(&source, &destination, false)).expect_err("412 returns error");
        assert_eq!(error.status_code(), Some(412));
        let status = error.as_status().expect("status error");
        assert_eq!(status.method(), "MOVE");
        assert_eq!(
            status.path().as_str(),
            "/remote.php/dav/files/alice/src.txt"
        );
    }

    #[test]
    fn get_range_sets_range_header_and_returns_bytes() {
        let transport = FakeTransport::new(206, "partial content bytes");
        let auth = FakeAuth;
        let client = DavClient::new(&transport, &auth);
        let path = DavPath::new("alice", "photo.jpg").expect("path is valid");

        let bytes = run(client.get_range(&path, 0, 1024)).expect("range request succeeds");
        assert_eq!(bytes, b"partial content bytes");

        let request = transport.last_request();
        assert_eq!(request.method(), HttpMethod::Get);
        assert_eq!(request.headers().get("Range"), Some("bytes=0-1024"));
    }
}

#[cfg(test)]
mod range_tests {
    use super::{ranged_start, window};
    use nexia_nextcloud_core::Headers;

    #[test]
    fn a_full_body_answer_is_reduced_to_the_requested_window() {
        let body = b"0123456789";
        assert_eq!(window(body, 2, 4), b"234");
    }

    #[test]
    fn a_window_past_the_end_is_clamped() {
        let body = b"0123456789";
        assert_eq!(window(body, 8, 99), b"89");
        assert!(window(body, 20, 30).is_empty());
    }

    #[test]
    fn a_content_range_start_is_read_back() {
        let mut headers = Headers::new();
        headers.set("Content-Range", "bytes 1024-2047/4096");
        assert_eq!(ranged_start(&headers), Some(1024));
    }

    #[test]
    fn a_missing_or_unreadable_content_range_yields_none() {
        let headers = Headers::new();
        assert_eq!(ranged_start(&headers), None);

        let mut headers = Headers::new();
        headers.set("Content-Range", "items 1-2");
        assert_eq!(ranged_start(&headers), None);
    }
}
