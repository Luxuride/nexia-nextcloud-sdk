//! Unit tests for TrashClient using a recorded fake transport.

use std::future::Future;
use std::sync::Mutex;
use std::task::Poll;

use nextcloud_auth::Anonymous;
use nextcloud_dav::{DEPTH_HEADER, DESTINATION_HEADER, OVERWRITE_HEADER};
use nextcloud_sdk_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use crate::TrashClient;

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

    fn seen(&self) -> Vec<Request> {
        self.seen.lock().expect("mutex is not poisoned").clone()
    }
}

impl Transport for FakeTransport {
    async fn execute(&self, request: Request) -> Result<Response, Error> {
        self.seen
            .lock()
            .expect("mutex is not poisoned")
            .push(request);
        Ok(Response::new(
            self.status,
            Headers::new(),
            self.body.clone(),
        ))
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

const SAMPLE_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/doc.pdf.d1600000000</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Thu, 01 Jan 2026 12:00:00 GMT</d:getlastmodified>
        <d:resourcetype/>
        <oc:trashbin-filename>doc.pdf</oc:trashbin-filename>
        <oc:trashbin-original-location>Documents/doc.pdf</oc:trashbin-original-location>
        <oc:trashbin-deletion-time>1600000000</oc:trashbin-deletion-time>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

#[test]
fn list_executes_propfind_and_decodes_items() {
    let transport = FakeTransport::new(207, SAMPLE_MULTISTATUS);
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let items = run(client.list("alice")).expect("list succeeds");
    assert_eq!(items.len(), 1);

    let item = &items[0];
    assert_eq!(item.id, "doc.pdf.d1600000000");
    assert_eq!(item.filename, "doc.pdf");
    assert_eq!(item.original_location, "Documents/doc.pdf");
    assert_eq!(item.deletion_time, Some(1600000000));
    assert_eq!(
        item.last_modified.as_deref(),
        Some("Thu, 01 Jan 2026 12:00:00 GMT")
    );
    assert!(!item.is_collection);

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.method(), HttpMethod::Propfind);
    assert_eq!(req.path().as_str(), "/remote.php/dav/trashbin/alice/trash/");
    assert_eq!(req.headers().get(DEPTH_HEADER), Some("1"));
    assert_eq!(
        req.headers().get("Content-Type"),
        Some("application/xml; charset=utf-8")
    );
    let body_bytes = req.body().expect("body is present");
    let body_str = std::str::from_utf8(body_bytes).expect("valid utf8");
    assert!(body_str.contains("<d:propfind"));
    assert!(body_str.contains("<oc:trashbin-filename/>"));
}

#[test]
fn list_returns_empty_when_no_deleted_items() {
    let empty_multistatus = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;
    let transport = FakeTransport::new(207, empty_multistatus);
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let items = run(client.list("alice")).expect("list succeeds");
    assert!(items.is_empty());
}

#[test]
fn list_fails_on_status_error() {
    let transport = FakeTransport::new(500, "Internal Server Error");
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let err = run(client.list("alice")).expect_err("status error");
    match err {
        Error::Status(status) => {
            assert_eq!(status.status(), StatusCode::new(500));
            assert_eq!(status.method(), "PROPFIND");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn list_fails_on_decode_error() {
    let transport = FakeTransport::new(207, "Not XML");
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let err = run(client.list("alice")).expect_err("decode error");
    assert!(matches!(err, Error::Decode(_)));
}

#[test]
fn list_rejects_invalid_user_id() {
    let transport = FakeTransport::new(207, SAMPLE_MULTISTATUS);
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    assert!(matches!(run(client.list("..")), Err(Error::Url(_))));
    assert!(matches!(run(client.list("alice/bob")), Err(Error::Url(_))));
    assert!(transport.seen().is_empty());
}

#[test]
fn restore_executes_move_with_destination_and_overwrite() {
    let transport = FakeTransport::new(201, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    run(client.restore("alice", "doc.pdf.d1600000000", "Documents/doc.pdf"))
        .expect("restore succeeds");

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.method(), HttpMethod::Move);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/trashbin/alice/trash/doc.pdf.d1600000000"
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/trashbin/alice/restore/Documents/doc.pdf")
    );
    assert_eq!(req.headers().get(OVERWRITE_HEADER), Some("T"));
}

#[test]
fn restore_to_default_restore_folder_when_path_is_empty() {
    let transport = FakeTransport::new(204, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    run(client.restore("alice", "doc.pdf.d1600000000", "")).expect("restore succeeds");

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/trashbin/alice/restore")
    );
}

#[test]
fn restore_fails_on_status_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let err =
        run(client.restore("alice", "doc.pdf.d1600000000", "doc.pdf")).expect_err("not found");
    assert!(matches!(err, Error::Status(_)));
}

#[test]
fn restore_rejects_invalid_inputs() {
    let transport = FakeTransport::new(201, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    assert!(matches!(
        run(client.restore("..", "doc", "dest")),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        run(client.restore("alice", "..", "dest")),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        run(client.restore("alice", "doc", "../traversal")),
        Err(Error::Url(_))
    ));
    assert!(transport.seen().is_empty());
}

#[test]
fn delete_executes_delete_on_item() {
    let transport = FakeTransport::new(204, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    run(client.delete("alice", "doc.pdf.d1600000000")).expect("delete succeeds");

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/trashbin/alice/trash/doc.pdf.d1600000000"
    );
}

#[test]
fn delete_fails_on_status_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let err = run(client.delete("alice", "missing.txt")).expect_err("not found");
    assert!(matches!(err, Error::Status(_)));
}

#[test]
fn empty_executes_delete_on_trash_root() {
    let transport = FakeTransport::new(204, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    run(client.empty("alice")).expect("empty succeeds");

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(req.path().as_str(), "/remote.php/dav/trashbin/alice/trash/");
}

#[test]
fn empty_fails_on_status_error() {
    let transport = FakeTransport::new(500, "Error");
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);

    let err = run(client.empty("alice")).expect_err("status error");
    assert!(matches!(err, Error::Status(_)));
}

struct TestAuth;

impl AuthProvider for TestAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", "Bearer test-token");
    }
}

#[test]
fn credentials_applied_to_requests() {
    let transport = FakeTransport::new(204, Vec::new());
    let auth = TestAuth;
    let client = TrashClient::new(&transport, &auth);

    assert_eq!(client.transport().status, StatusCode::new(204));
    let _ = client.auth();

    run(client.empty("alice")).expect("empty succeeds");

    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].headers().get("Authorization"),
        Some("Bearer test-token")
    );
}

#[test]
fn client_display() {
    let transport = FakeTransport::new(200, Vec::new());
    let auth = Anonymous;
    let client = TrashClient::new(&transport, &auth);
    assert_eq!(client.to_string(), "TrashClient");
}
