use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use sdk_core::{ApiPath, AuthProvider, Error, Headers, Request, Response, StatusCode, Transport};
use serde::{Deserialize, Serialize};

use super::OcsClient;
use crate::query::{OcsFormat, Query};

/// Drives a future to completion on the current thread.
///
/// The transport below never yields, so a no-op waker and a spin on `Pending`
/// are enough and the crate needs no async runtime dependency.
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

/// Canned transport: records the request it was handed and answers with a
/// fixed response, so no test reaches the network.
#[derive(Clone, Debug)]
struct FakeTransport {
    response: Response,
    seen: Arc<Mutex<Option<Request>>>,
}

impl FakeTransport {
    fn new(status: u16, body: &str) -> Self {
        Self {
            response: Response::new(StatusCode::new(status), Headers::new(), body.as_bytes()),
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
        Ok(self.response.clone())
    }
}

/// Stamps a credential header the way a real provider does.
#[derive(Clone, Debug)]
struct FakeAuth;

impl AuthProvider for FakeAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", "Bearer test-token");
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct Capabilities {
    version: Version,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct Version {
    major: u8,
    string: String,
}

const CAPABILITIES_OK: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK",
      "totalitems": "",
      "itemsperpage": ""
    },
    "data": {
      "version": { "major": 32, "string": "32.0.0" }
    }
  }
}"#;

const OCS_FAILURE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 997,
      "message": "No app exists with the name \"nonexistent\""
    },
    "data": []
  }
}"#;

fn path() -> ApiPath {
    ApiPath::ocs("cloud/capabilities").expect("valid OCS path")
}

fn client(transport: FakeTransport) -> OcsClient<FakeTransport, FakeAuth> {
    OcsClient::new(transport, FakeAuth)
}

#[test]
fn ok_response_yields_the_payload() {
    let transport = FakeTransport::new(200, CAPABILITIES_OK);
    let capabilities: Capabilities = block_on(
        client(transport.clone())
            .get(path(), Query::new())
            .into_future(),
    )
    .expect("the envelope carries a payload");

    assert_eq!(capabilities.version.major, 32);
    assert_eq!(capabilities.version.string, "32.0.0");
    assert!(transport.request().is_some());
}

#[test]
fn request_carries_ocs_headers_and_credentials() {
    let transport = FakeTransport::new(200, CAPABILITIES_OK);
    let _: Capabilities = block_on(
        client(transport.clone())
            .get(path(), Query::new())
            .into_future(),
    )
    .expect("payload decodes");

    let request = transport.request().expect("transport saw a request");
    assert_eq!(request.headers().get("Accept"), Some("application/json"));
    assert_eq!(request.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(
        request.headers().get("Authorization"),
        Some("Bearer test-token")
    );
    assert_eq!(
        request.path().as_str(),
        "/ocs/v2.php/cloud/capabilities",
        "an empty query leaves no trailing separator"
    );
}

#[test]
fn query_format_reaches_the_request_path() {
    let transport = FakeTransport::new(200, CAPABILITIES_OK);
    let _: Capabilities = block_on(
        client(transport.clone())
            .get(path(), Query::new().format(OcsFormat::Json))
            .into_future(),
    )
    .expect("payload decodes");

    assert_eq!(
        transport
            .request()
            .map(|request| request.path().as_str().to_owned()),
        Some("/ocs/v2.php/cloud/capabilities?format=json".to_owned())
    );
}

#[test]
fn failing_statuscode_becomes_a_status_error() {
    let transport = FakeTransport::new(200, OCS_FAILURE);
    let error = block_on(
        client(transport)
            .get::<Capabilities>(path(), Query::new())
            .into_future(),
    )
    .expect_err("meta.statuscode 997 is a failure");

    assert!(
        error.as_status().is_some(),
        "the OCS verdict fails the call"
    );
    assert_eq!(error.status_code(), Some(200));
    assert!(
        error.body().unwrap_or_default().contains("997"),
        "the OCS body stays reachable so the code and message survive"
    );
}

#[test]
fn non_2xx_becomes_a_status_error() {
    let transport = FakeTransport::new(503, "");
    let error = block_on(
        client(transport)
            .get::<Capabilities>(path(), Query::new())
            .into_future(),
    )
    .expect_err("503 is not acceptable");

    assert!(error.as_status().is_some());
    assert_eq!(error.status_code(), Some(503));
    assert!(error.is_retryable());
}

#[test]
fn undecodable_body_names_the_target_type() {
    let transport = FakeTransport::new(200, r#"{"not_ocs": true}"#);
    let error = block_on(
        client(transport)
            .get::<Capabilities>(path(), Query::new())
            .into_future(),
    )
    .expect_err("a missing `ocs` wrapper cannot decode");

    let decode = error.as_decode().expect("a decode failure");
    assert!(decode.target().contains("OcsEnvelope"));
    assert!(!error.is_retryable());
}

#[test]
fn envelope_without_a_verdict_is_a_decode_error() {
    let transport = FakeTransport::new(200, r#"{"ocs":{"meta":{},"data":null}}"#);
    let error = block_on(
        client(transport)
            .get::<Capabilities>(path(), Query::new())
            .into_future(),
    )
    .expect_err("no verdict is not a success");

    assert!(error.as_decode().is_some());
}

#[test]
fn non_ocs_paths_are_refused_before_sending() {
    let transport = FakeTransport::new(200, CAPABILITIES_OK);
    let error = block_on(
        client(transport.clone())
            .get::<Capabilities>(
                ApiPath::dav("files/user").expect("valid DAV path"),
                Query::new(),
            )
            .into_future(),
    )
    .expect_err("a DAV path is not an OCS call");

    assert!(error.as_url().is_some());
    assert!(transport.request().is_none(), "nothing was sent");
}
