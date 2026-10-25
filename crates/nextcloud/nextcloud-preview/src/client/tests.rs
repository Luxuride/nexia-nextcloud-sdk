use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nextcloud_sdk_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use super::PreviewClient;
use crate::mode::PreviewMode;
use crate::options::PreviewOptions;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[derive(Clone, Debug)]
struct FakeTransport {
    result: Arc<Mutex<Result<Response, String>>>,
    seen: Arc<Mutex<Option<Request>>>,
}

impl FakeTransport {
    fn with_success(status: u16, body: Vec<u8>) -> Self {
        Self {
            result: Arc::new(Mutex::new(Ok(Response::new(
                StatusCode::new(status),
                Headers::new(),
                body,
            )))),
            seen: Arc::new(Mutex::new(None)),
        }
    }

    fn with_failure(error_msg: &str) -> Self {
        Self {
            result: Arc::new(Mutex::new(Err(error_msg.to_owned()))),
            seen: Arc::new(Mutex::new(None)),
        }
    }

    fn request(&self) -> Option<Request> {
        self.seen.lock().ok().and_then(|seen| seen.clone())
    }
}

impl Transport for FakeTransport {
    async fn execute(&self, request: Request) -> Result<Response, Error> {
        if let Ok(mut seen) = self.seen.lock() {
            *seen = Some(request);
        }
        match self.result.lock().unwrap().clone() {
            Ok(resp) => Ok(resp),
            Err(msg) => Err(Error::transport(std::io::Error::other(msg))),
        }
    }
}

#[derive(Clone, Debug)]
struct FakeAuth {
    token: &'static str,
}

impl FakeAuth {
    const fn new(token: &'static str) -> Self {
        Self { token }
    }
}

impl AuthProvider for FakeAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", format!("Bearer {}", self.token));
    }
}

const FAKE_PNG_BYTES: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01];

#[test]
fn get_by_file_id_sends_correct_url_and_headers() {
    let transport = FakeTransport::with_success(200, FAKE_PNG_BYTES.to_vec());
    let auth = FakeAuth::new("preview-secret-token");
    let client = PreviewClient::new(&transport, &auth);

    let options = PreviewOptions::new()
        .width(128)
        .height(256)
        .keep_aspect_ratio(true)
        .mode(PreviewMode::Cover);

    let result = block_on(client.get_by_file_id(42, &options));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FAKE_PNG_BYTES);

    let req = transport
        .request()
        .expect("request must have been recorded");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/index.php/core/preview.png?fileId=42&x=128&y=256&a=1&mode=cover"
    );

    assert_eq!(
        req.headers().get("authorization"),
        Some("Bearer preview-secret-token")
    );
}

#[test]
fn get_by_file_id_with_fill_mode_and_no_aspect_ratio() {
    let transport = FakeTransport::with_success(200, FAKE_PNG_BYTES.to_vec());
    let auth = FakeAuth::new("token123");
    let client = PreviewClient::new(&transport, &auth);

    let options = PreviewOptions::new()
        .dimensions(500, 500)
        .keep_aspect_ratio(false)
        .mode(PreviewMode::Fill);

    let result = block_on(client.get_by_file_id(1001, &options));
    assert!(result.is_ok());

    let req = transport
        .request()
        .expect("request must have been recorded");
    assert_eq!(
        req.path().as_str(),
        "/index.php/core/preview.png?fileId=1001&x=500&y=500&a=0&mode=fill"
    );
}

#[test]
fn get_by_file_id_minimal_options() {
    let transport = FakeTransport::with_success(200, FAKE_PNG_BYTES.to_vec());
    let auth = FakeAuth::new("token123");
    let client = PreviewClient::new(&transport, &auth);

    let options = PreviewOptions::new();
    let result = block_on(client.get_by_file_id(77, &options));
    assert!(result.is_ok());

    let req = transport
        .request()
        .expect("request must have been recorded");
    assert_eq!(req.path().as_str(), "/index.php/core/preview.png?fileId=77");
}

#[test]
fn get_by_path_sends_encoded_path_and_options() {
    let transport = FakeTransport::with_success(200, FAKE_PNG_BYTES.to_vec());
    let auth = FakeAuth::new("token-abc");
    let client = PreviewClient::new(&transport, &auth);

    let options = PreviewOptions::new().width(64).height(64);

    let result = block_on(client.get_by_path("Photos/summer vacation.jpg", &options));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FAKE_PNG_BYTES);

    let req = transport
        .request()
        .expect("request must have been recorded");
    assert_eq!(req.method(), HttpMethod::Get);
    assert_eq!(
        req.path().as_str(),
        "/index.php/core/preview.png?file=Photos%2Fsummer%20vacation.jpg&x=64&y=64"
    );

    assert_eq!(req.headers().get("authorization"), Some("Bearer token-abc"));
}

#[test]
fn get_by_path_with_leading_slash_and_unicode() {
    let transport = FakeTransport::with_success(200, FAKE_PNG_BYTES.to_vec());
    let auth = FakeAuth::new("token-abc");
    let client = PreviewClient::new(&transport, &auth);

    let options = PreviewOptions::new().mode(PreviewMode::Cover);
    let result = block_on(client.get_by_path("/Bilder/münchen.png", &options));
    assert!(result.is_ok());

    let req = transport.request().expect("request recorded");
    assert_eq!(
        req.path().as_str(),
        "/index.php/core/preview.png?file=%2FBilder%2Fm%C3%BCnchen.png&mode=cover"
    );
}

#[test]
fn returns_status_error_on_404() {
    let transport = FakeTransport::with_success(404, b"File not found".to_vec());
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    let result = block_on(client.get_by_file_id(999, &PreviewOptions::new()));
    match result {
        Err(Error::Status(status_err)) => {
            assert_eq!(status_err.status(), StatusCode::new(404));
            assert_eq!(status_err.method(), "GET");
            assert_eq!(status_err.body(), "File not found");
        }
        other => panic!("expected Error::Status, got {other:?}"),
    }
}

#[test]
fn returns_status_error_on_500() {
    let transport = FakeTransport::with_success(500, b"Internal server error".to_vec());
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    let result = block_on(client.get_by_path("test.png", &PreviewOptions::new()));
    match result {
        Err(Error::Status(status_err)) => {
            assert_eq!(status_err.status(), StatusCode::new(500));
            assert_eq!(status_err.method(), "GET");
        }
        other => panic!("expected Error::Status, got {other:?}"),
    }
}

#[test]
fn propagates_transport_error() {
    let transport = FakeTransport::with_failure("connection reset");
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    let result = block_on(client.get_by_file_id(1, &PreviewOptions::new()));
    assert!(matches!(result, Err(Error::Transport(_))));
}

#[test]
fn client_accessors() {
    let transport = FakeTransport::with_success(200, vec![]);
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    assert_eq!(client.auth().token, "token");
    assert!(client.transport().request().is_none());
}

#[test]
fn falls_back_to_core_preview_when_png_endpoint_returns_404() {
    #[derive(Clone, Debug)]
    struct FallbackTransport;

    impl Transport for FallbackTransport {
        async fn execute(&self, request: Request) -> Result<Response, Error> {
            if request.path().as_str().contains("core/preview.png") {
                Ok(Response::new(
                    StatusCode::new(404),
                    Headers::new(),
                    b"Not Found".to_vec(),
                ))
            } else if request.path().as_str().contains("core/preview") {
                Ok(Response::new(
                    StatusCode::new(200),
                    Headers::new(),
                    FAKE_PNG_BYTES.to_vec(),
                ))
            } else {
                Ok(Response::new(
                    StatusCode::new(404),
                    Headers::new(),
                    Vec::new(),
                ))
            }
        }
    }

    let transport = FallbackTransport;
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    let result = block_on(client.get_by_file_id(42, &PreviewOptions::new()));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FAKE_PNG_BYTES);
}

#[test]
fn falls_back_to_core_preview_when_png_endpoint_returns_400() {
    #[derive(Clone, Debug)]
    struct Fallback400Transport;

    impl Transport for Fallback400Transport {
        async fn execute(&self, request: Request) -> Result<Response, Error> {
            if request.path().as_str().contains("core/preview.png") {
                Ok(Response::new(
                    StatusCode::new(400),
                    Headers::new(),
                    b"Bad Request".to_vec(),
                ))
            } else if request.path().as_str().contains("core/preview") {
                Ok(Response::new(
                    StatusCode::new(200),
                    Headers::new(),
                    FAKE_PNG_BYTES.to_vec(),
                ))
            } else {
                Ok(Response::new(
                    StatusCode::new(404),
                    Headers::new(),
                    Vec::new(),
                ))
            }
        }
    }

    let transport = Fallback400Transport;
    let auth = FakeAuth::new("token");
    let client = PreviewClient::new(&transport, &auth);

    let result = block_on(client.get_by_file_id(42, &PreviewOptions::new()));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FAKE_PNG_BYTES);
}
