use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nexia_nextcloud_core::{
    AuthProvider, Error, Headers, Request, Response, StatusCode, Transport,
};
use nextcloud_ocs::OcsClient;

use super::SearchClient;

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

#[derive(Clone, Debug)]
struct FakeAuth;

impl AuthProvider for FakeAuth {
    fn authenticate(&self, headers: &mut Headers) {
        headers.set("Authorization", "Bearer search-test-token");
    }
}

const PROVIDERS_ENVELOPE: &str = r##"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": [
      {
        "id": "files",
        "name": "Files",
        "icon": "icon-files",
        "order": 10
      },
      {
        "id": "bookmarks",
        "name": "Bookmarks",
        "icon": "icon-bookmark",
        "order": 20
      }
    ]
  }
}"##;

const SEARCH_RESULTS_ENVELOPE: &str = r##"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "name": "Files",
      "isPaginated": false,
      "entries": [
        {
          "thumbnailUrl": "/index.php/core/preview.png?fileId=42&x=32&y=32",
          "title": "QuarterlyReport.pdf",
          "subline": "/Documents/QuarterlyReport.pdf",
          "resourceUrl": "https://nextcloud.example.com/f/42",
          "icon": "icon-file-pdf",
          "rounded": false,
          "attributes": {
            "size": 1048576
          }
        }
      ],
      "cursor": null
    }
  }
}"##;

#[test]
fn list_providers_decodes_successfully() {
    let transport = FakeTransport::new(200, PROVIDERS_ENVELOPE);
    let auth = FakeAuth;
    let ocs = OcsClient::new(transport.clone(), auth);
    let client = SearchClient::new(&ocs);

    let providers = block_on(client.list_providers()).expect("list providers");
    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].id, "files");
    assert_eq!(providers[0].name, "Files");
    assert_eq!(providers[0].order, 10);
    assert_eq!(providers[1].id, "bookmarks");

    let req = transport.request().expect("recorded request");
    assert_eq!(
        req.path().as_str(),
        "/ocs/v2.php/search/providers?format=json"
    );
}

#[test]
fn search_with_term_decodes_successfully() {
    let transport = FakeTransport::new(200, SEARCH_RESULTS_ENVELOPE);
    let auth = FakeAuth;
    let ocs = OcsClient::new(transport.clone(), auth);
    let client = SearchClient::new(&ocs);

    let result = block_on(client.search("files", "Quarterly")).expect("search files");
    assert_eq!(result.name, "Files");
    assert!(!result.is_paginated);
    assert_eq!(result.entries.len(), 1);

    let entry = &result.entries[0];
    assert_eq!(entry.title, "QuarterlyReport.pdf");
    assert_eq!(
        entry.subline.as_deref(),
        Some("/Documents/QuarterlyReport.pdf")
    );
    assert_eq!(entry.icon.as_deref(), Some("icon-file-pdf"));
    assert_eq!(
        entry.thumbnail_url.as_deref(),
        Some("/index.php/core/preview.png?fileId=42&x=32&y=32")
    );

    let req = transport.request().expect("recorded request");
    assert_eq!(
        req.path().as_str(),
        "/ocs/v2.php/search/providers/files/search?format=json&term=Quarterly"
    );
}

#[test]
fn search_with_limit_and_cursor() {
    let transport = FakeTransport::new(200, SEARCH_RESULTS_ENVELOPE);
    let auth = FakeAuth;
    let ocs = OcsClient::new(transport.clone(), auth);
    let client = SearchClient::new(&ocs);

    let _ = block_on(client.search_with_options("files", "test", Some(10), Some(5)))
        .expect("search with options");

    let req = transport.request().expect("recorded request");
    assert_eq!(
        req.path().as_str(),
        "/ocs/v2.php/search/providers/files/search?format=json&term=test&limit=10&cursor=5"
    );
}
