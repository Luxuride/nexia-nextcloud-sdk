use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

use nexia_nextcloud_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use super::{MTIME_HEADER, TOTAL_LENGTH_HEADER, UploadClient};
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
        headers.set("Authorization", "Bearer upload-token-123");
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

#[test]
fn start_upload_success() {
    let transport = FakeTransport::new(201, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = block_on(client.start("alice", "Documents/report.pdf", 15_000_000))
        .expect("start succeeds");

    assert_eq!(session.user_id(), "alice");
    assert!(session.upload_id().starts_with("chunked-upload-"));
    assert_eq!(
        session.destination_path(),
        "/remote.php/dav/files/alice/Documents/report.pdf"
    );
    assert_eq!(session.total_size(), 15_000_000);

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Mkcol);
    assert!(
        req.path()
            .as_str()
            .starts_with("/remote.php/dav/uploads/alice/chunked-upload-")
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/files/alice/Documents/report.pdf")
    );
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer upload-token-123")
    );
}

#[test]
fn start_with_id_success() {
    let transport = FakeTransport::new(201, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = block_on(client.start_with_id(
        "alice",
        "/remote.php/dav/files/alice/Documents/report.pdf",
        20_000_000,
        "custom-upload-999",
    ))
    .expect("start_with_id succeeds");

    assert_eq!(session.upload_id(), "custom-upload-999");
    assert_eq!(
        session.destination_path(),
        "/remote.php/dav/files/alice/Documents/report.pdf"
    );

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Mkcol);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/uploads/alice/custom-upload-999"
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/files/alice/Documents/report.pdf")
    );
}

#[test]
fn start_status_error() {
    let transport = FakeTransport::new(403, "Forbidden");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let err = block_on(client.start("alice", "file.bin", 1000)).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 403);
            assert_eq!(status_err.method(), "MKCOL");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn start_invalid_arguments() {
    let transport = FakeTransport::new(201, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    assert!(matches!(
        block_on(client.start("alice/danger", "file.bin", 1000)),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        block_on(client.start("alice", "../file.bin", 1000)),
        Err(Error::Url(_))
    ));
    assert!(matches!(
        block_on(client.start_with_id("alice", "file.bin", 1000, "upload/slash")),
        Err(Error::Url(_))
    ));
}

#[test]
fn upload_chunk_success() {
    let transport = FakeTransport::new(201, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session =
        block_on(client.start_with_id("alice", "video.mp4", 15_000_000, "up-session-1")).unwrap();

    let chunk_data = vec![42u8; 5 * 1024 * 1024];
    block_on(client.upload_chunk(&session, 1, chunk_data.clone())).expect("upload_chunk succeeds");

    assert!(session.is_chunk_uploaded(1));
    assert_eq!(session.uploaded_chunk_count(), 1);

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Put);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/uploads/alice/up-session-1/00001"
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/files/alice/video.mp4")
    );
    assert_eq!(req.headers().get(TOTAL_LENGTH_HEADER), Some("15000000"));
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer upload-token-123")
    );
    assert_eq!(req.body(), Some(&chunk_data[..]));
}

#[test]
fn upload_chunk_status_error() {
    let transport = FakeTransport::new(500, "Internal Server Error");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new("alice", "up-1", "/dest", 1000);
    let err = block_on(client.upload_chunk(&session, 1, vec![1, 2, 3])).unwrap_err();

    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 500);
            assert_eq!(status_err.method(), "PUT");
        }
        other => panic!("expected status error, got {other:?}"),
    }
    assert!(!session.is_chunk_uploaded(1));
}

#[test]
fn upload_chunk_invalid_index() {
    let transport = FakeTransport::new(201, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new("alice", "up-1", "/dest", 1000);
    let err = block_on(client.upload_chunk(&session, 0, vec![1, 2])).unwrap_err();
    assert!(matches!(err, Error::Url(_)));
}

#[test]
fn finish_success_with_and_without_mtime() {
    let transport = FakeTransport::new(204, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new(
        "alice",
        "up-123",
        "/remote.php/dav/files/alice/Photos/pic.png",
        20_000_000,
    );

    // Finish with mtime
    block_on(client.finish(&session, Some(1729789200))).expect("finish with mtime succeeds");
    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Move);
    assert_eq!(
        req.path().as_str(),
        "/remote.php/dav/uploads/alice/up-123/.file"
    );
    assert_eq!(
        req.headers().get(DESTINATION_HEADER),
        Some("/remote.php/dav/files/alice/Photos/pic.png")
    );
    assert_eq!(req.headers().get(TOTAL_LENGTH_HEADER), Some("20000000"));
    assert_eq!(req.headers().get(MTIME_HEADER), Some("1729789200"));
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer upload-token-123")
    );

    // Finish without mtime
    block_on(client.finish(&session, None)).expect("finish without mtime succeeds");
    let req2 = transport.last_request();
    assert_eq!(req2.method(), HttpMethod::Move);
    assert_eq!(req2.headers().get(MTIME_HEADER), None);
}

#[test]
fn finish_status_error() {
    let transport = FakeTransport::new(409, "Conflict");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new("alice", "up-1", "/dest", 1000);
    let err = block_on(client.finish(&session, None)).unwrap_err();

    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 409);
            assert_eq!(status_err.method(), "MOVE");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn abort_success() {
    let transport = FakeTransport::new(204, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new("alice", "up-123", "/dest", 1000);
    block_on(client.abort(&session)).expect("abort succeeds");

    let req = transport.last_request();
    assert_eq!(req.method(), HttpMethod::Delete);
    assert_eq!(req.path().as_str(), "/remote.php/dav/uploads/alice/up-123/");
    assert_eq!(req.headers().get(DESTINATION_HEADER), None);
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer upload-token-123")
    );
}

#[test]
fn abort_status_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    let session = crate::session::UploadSession::new("alice", "up-123", "/dest", 1000);
    let err = block_on(client.abort(&session)).unwrap_err();

    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status().as_u16(), 404);
            assert_eq!(status_err.method(), "DELETE");
        }
        other => panic!("expected status error, got {other:?}"),
    }
}

#[test]
fn client_accessors_and_display() {
    let transport = FakeTransport::new(200, "");
    let auth = FakeAuth;
    let client = UploadClient::new(&transport, &auth);

    assert_eq!(client.transport().status.as_u16(), 200);
    let mut headers = Headers::new();
    client.auth().authenticate(&mut headers);
    assert_eq!(
        headers.get("Authorization"),
        Some("Bearer upload-token-123")
    );
    assert_eq!(client.to_string(), "UploadClient");
}
