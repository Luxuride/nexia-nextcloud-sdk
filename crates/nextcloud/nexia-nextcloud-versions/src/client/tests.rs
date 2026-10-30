use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use nexia_nextcloud_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use super::VersionsClient;
use nexia_nextcloud_dav::DESTINATION_HEADER;

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
        let seen = self.seen.lock().expect("test mutex not poisoned");
        seen.last().cloned().expect("a request was made")
    }
}

impl Transport for FakeTransport {
    async fn execute(&self, request: Request) -> Result<Response, Error> {
        self.seen
            .lock()
            .expect("test mutex not poisoned")
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
        headers.set("Authorization", "Bearer token-abc");
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

const SAMPLE_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/versions/alice/versions/42/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/versions/alice/versions/42/1609459200</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Fri, 01 Jan 2021 00:00:00 GMT</d:getlastmodified>
        <d:getcontentlength>512</d:getcontentlength>
        <d:getetag>"rev-etag-1"</d:getetag>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

#[test]
fn list_success() {
    let transport = FakeTransport::new(207, SAMPLE_MULTISTATUS);
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let versions = block_on(client.list("alice", 42)).expect("list succeeds");
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].revision_id, "1609459200");
    assert_eq!(versions[0].content_length, Some(512));
    assert_eq!(versions[0].etag.as_deref(), Some("\"rev-etag-1\""));
    assert_eq!(
        versions[0].last_modified.as_deref(),
        Some("Fri, 01 Jan 2021 00:00:00 GMT")
    );

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Propfind);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/versions/alice/versions/42"
    );
    assert_eq!(req.headers().get("Depth"), Some("1"));
    assert_eq!(req.headers().get("Authorization"), Some("Bearer token-abc"));
    assert!(req.headers().get("Content-Type").unwrap().contains("xml"));
}

#[test]
fn list_status_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let err = block_on(client.list("alice", 42)).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 404);
            assert_eq!(status_err.method(), "PROPFIND");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn list_invalid_user() {
    let transport = FakeTransport::new(207, SAMPLE_MULTISTATUS);
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let err = block_on(client.list("alice/danger", 42)).unwrap_err();
    assert!(matches!(err, Error::Url(_)));
}

#[test]
fn get_success() {
    let file_bytes = b"revision content bytes";
    let transport = FakeTransport::new(200, file_bytes.to_vec());
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let bytes = block_on(client.get("alice", 42, "1609459200")).expect("get succeeds");
    assert_eq!(bytes, file_bytes);

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/versions/alice/versions/42/1609459200"
    );
    assert_eq!(req.headers().get("Authorization"), Some("Bearer token-abc"));
}

#[test]
fn get_status_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let err = block_on(client.get("alice", 42, "1609459200")).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 404);
            assert_eq!(status_err.method(), "GET");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn get_invalid_args() {
    let transport = FakeTransport::new(200, b"".to_vec());
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    assert!(matches!(
        block_on(client.get("../alice", 42, "1609459200")),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        block_on(client.get("alice", 42, "1609459200/extra")),
        Err(Error::Url(_))
    ));
}

#[test]
fn restore_success() {
    let transport = FakeTransport::new(204, "");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    block_on(client.restore("alice", 42, "1609459200")).expect("restore succeeds");

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Move);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/versions/alice/versions/42/1609459200"
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/versions/alice/restore/42")
    );
    assert_eq!(req.headers().get("Authorization"), Some("Bearer token-abc"));
}

#[test]
fn restore_status_error() {
    let transport = FakeTransport::new(409, "Conflict");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    let err = block_on(client.restore("alice", 42, "1609459200")).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 409);
            assert_eq!(status_err.method(), "MOVE");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn restore_invalid_args() {
    let transport = FakeTransport::new(204, "");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    assert!(matches!(
        block_on(client.restore("alice?", 42, "1609459200")),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        block_on(client.restore("alice", 42, "..")),
        Err(Error::Url(_))
    ));
}

#[test]
fn client_accessors() {
    let transport = FakeTransport::new(200, "");
    let auth = FakeAuth;
    let client = VersionsClient::new(&transport, &auth);

    assert_eq!(client.transport().status.as_u16(), 200);
    let mut headers = Headers::new();
    client.auth().authenticate(&mut headers);
    assert_eq!(headers.get("Authorization"), Some("Bearer token-abc"));
}
