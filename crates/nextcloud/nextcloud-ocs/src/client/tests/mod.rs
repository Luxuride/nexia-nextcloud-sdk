use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nexia_nextcloud_core::{
    ApiPath, AuthProvider, Error, Headers, Request, Response, StatusCode, Transport,
};
use serde::{Deserialize, Serialize};

use super::OcsClient;

mod get;
mod mutation;
mod serialization;
mod unit_response;

/// Drives a future to completion on the current thread.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
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
pub(crate) struct FakeTransport {
    response: Response,
    seen: Arc<Mutex<Option<Request>>>,
}

impl FakeTransport {
    pub(crate) fn new(status: u16, body: &str) -> Self {
        Self {
            response: Response::new(StatusCode::new(status), Headers::new(), body.as_bytes()),
            seen: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn request(&self) -> Option<Request> {
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
pub(crate) struct FakeAuth;

impl AuthProvider for FakeAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", "Bearer test-token");
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Capabilities {
    pub(crate) version: Version,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) struct Version {
    pub(crate) major: u8,
    pub(crate) string: String,
}

pub(crate) const CAPABILITIES_OK: &str = r#"{
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

pub(crate) const OCS_FAILURE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 997,
      "message": "No app exists with the name \"nonexistent\""
    },
    "data": []
  }
}"#;

pub(crate) fn capabilities_path() -> ApiPath {
    ApiPath::ocs("cloud/capabilities").expect("valid OCS path")
}

pub(crate) fn client(transport: FakeTransport) -> OcsClient<FakeTransport, FakeAuth> {
    OcsClient::new(transport, FakeAuth)
}
