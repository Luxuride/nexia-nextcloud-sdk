//! Request plumbing for WebDAV calls.

use sdk_core::{
    AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, StatusCode, StatusError,
    Transport,
};

use super::multistatus;
use crate::method::{DEPTH_HEADER, Depth};
use crate::path::DavPath;
use crate::propfind::PropfindBody;
use crate::resource::DavResource;

/// MIME type every WebDAV body in this crate is sent with.
const XML_CONTENT_TYPE: &str = "application/xml; charset=utf-8";

/// WebDAV client generic over the transport and the credential source.
///
/// Both are held by reference, so a client can be borrowed into several
/// spawned tasks and share one connection pool and one token.
#[derive(Clone, Copy, Debug)]
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
        let request = self.build_propfind(path, depth, body);
        let response = self.transport.execute(request).await?;

        if response.status() != StatusCode::new(207) {
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

    fn build_propfind(&self, path: &DavPath, depth: Depth, body: &PropfindBody) -> Request {
        // Credentials go on first: the headers set afterwards can then never
        // displace an `Authorization` the provider applied.
        let mut headers = Headers::new();
        self.auth.authenticate(&mut headers);
        headers.set(DEPTH_HEADER, depth.as_header_value());
        headers.set("Content-Type", XML_CONTENT_TYPE);

        Request::new(HttpMethod::Propfind, path.to_api_path())
            .with_headers(headers)
            .body_bytes(body.to_xml().into_bytes())
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

    use sdk_core::{
        AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
    };

    use super::{DavClient, XML_CONTENT_TYPE};
    use crate::fixtures::TWO_ENTRIES;
    use crate::method::{DEPTH_HEADER, Depth};
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
}
