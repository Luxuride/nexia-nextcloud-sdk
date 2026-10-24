use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nextcloud_ocs::OcsClient;
use sdk_core::{AuthProvider, Error, Headers, Request, Response, StatusCode, Transport};

use super::UserClient;

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
        headers.set("Authorization", "Bearer users-test-token");
    }
}

const CAPABILITIES_ENVELOPE: &str = r##"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "version": {
        "major": 30,
        "minor": 0,
        "micro": 4,
        "string": "30.0.4",
        "edition": "Community",
        "extendedSupport": false
      },
      "capabilities": {
        "core": {
          "pollinterval": 60,
          "webdav-root": "remote.php/webdav",
          "status": {
            "installed": true,
            "maintenance": false,
            "needsDbUpgrade": false,
            "versionstring": "30.0.4"
          }
        },
        "files": {
          "bigfilechunking": true,
          "undelete": true,
          "versioning": true,
          "chunked_upload": {
            "max_size": 104857600
          }
        },
        "theming": {
          "name": "Nextcloud Enterprise",
          "color": "#0082c9"
        }
      }
    }
  }
}"##;

const CURRENT_USER_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "enabled": true,
      "id": "alice",
      "display-name": "Alice Smith",
      "email": "alice@example.com",
      "storageLocation": "/var/www/nextcloud/data/alice",
      "backend": "Database",
      "groups": ["admin", "users"],
      "quota": {
        "free": 71373287424,
        "used": 694711965,
        "total": 72067999389,
        "relative": 0.96
      }
    }
  }
}"#;

const PROVISIONED_USER_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "enabled": true,
      "id": "bob",
      "displayname": "Bob Jones",
      "email": "bob@example.com",
      "home": "/var/www/nextcloud/data/bob",
      "backend": "LDAP",
      "groups": ["users"],
      "subadmin": ["group-1"],
      "quota": {
        "free": -3,
        "used": 1024,
        "total": -3,
        "relative": 0
      }
    }
  }
}"#;

const OCS_ERROR_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 997,
      "message": "User does not exist"
    },
    "data": []
  }
}"#;

#[test]
fn capabilities_fetches_and_decodes_server_capabilities() {
    let transport = FakeTransport::new(200, CAPABILITIES_ENVELOPE);
    let ocs = OcsClient::new(transport.clone(), FakeAuth);
    let client = UserClient::new(&ocs);

    let caps = block_on(client.capabilities()).expect("capabilities call succeeds");

    assert_eq!(caps.server_version(), Some("30.0.4"));
    assert_eq!(caps.core().poll_interval(), Some(60));
    assert_eq!(caps.core().webdav_root(), Some("remote.php/webdav"));
    assert!(caps.files().big_file_chunking());
    assert_eq!(caps.files().max_chunk_size(), Some(104857600));
    assert_eq!(
        caps.theming().and_then(|t| t.name()),
        Some("Nextcloud Enterprise")
    );

    let req = transport.request().expect("request was sent");
    assert_eq!(
        req.path().as_str(),
        "/ocs/v2.php/cloud/capabilities?format=json"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Accept"), Some("application/json"));
    assert_eq!(
        req.headers().get("Authorization"),
        Some("Bearer users-test-token")
    );
}

#[test]
fn current_user_fetches_and_decodes_profile() {
    let transport = FakeTransport::new(200, CURRENT_USER_ENVELOPE);
    let ocs = OcsClient::new(transport.clone(), FakeAuth);
    let client = UserClient::new(&ocs);

    let user = block_on(client.current_user()).expect("current_user succeeds");

    assert_eq!(user.id(), "alice");
    assert_eq!(user.display_name(), "Alice Smith");
    assert_eq!(user.email(), Some("alice@example.com"));
    assert!(user.is_enabled());
    assert_eq!(user.quota().used(), 694711965);
    assert_eq!(user.groups(), &["admin", "users"]);

    let req = transport.request().expect("request was sent");
    assert_eq!(req.path().as_str(), "/ocs/v2.php/cloud/user?format=json");
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Accept"), Some("application/json"));
}

#[test]
fn get_user_by_id_formats_path_and_decodes() {
    let transport = FakeTransport::new(200, PROVISIONED_USER_ENVELOPE);
    let ocs = OcsClient::new(transport.clone(), FakeAuth);
    let client = UserClient::new(&ocs);

    let user = block_on(client.get_user("bob")).expect("get_user succeeds");

    assert_eq!(user.id(), "bob");
    assert_eq!(user.display_name(), "Bob Jones");
    assert_eq!(user.email(), Some("bob@example.com"));
    assert!(user.quota().is_unlimited());
    assert_eq!(user.subadmin(), &["group-1"]);

    let req = transport.request().expect("request was sent");
    assert_eq!(
        req.path().as_str(),
        "/ocs/v2.php/cloud/users/bob?format=json"
    );
    assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(req.headers().get("Accept"), Some("application/json"));
}

#[test]
fn get_user_rejects_path_traversal() {
    let transport = FakeTransport::new(200, PROVISIONED_USER_ENVELOPE);
    let ocs = OcsClient::new(transport.clone(), FakeAuth);
    let client = UserClient::new(&ocs);

    let err = block_on(client.get_user("../admin")).expect_err("path traversal should fail");
    assert!(err.as_url().is_some());
    assert!(transport.request().is_none(), "no request was sent");
}

#[test]
fn handles_ocs_error_envelope() {
    let transport = FakeTransport::new(200, OCS_ERROR_ENVELOPE);
    let ocs = OcsClient::new(transport, FakeAuth);
    let client = UserClient::new(&ocs);

    let err = block_on(client.get_user("nonexistent")).expect_err("OCS failure statuscode");
    assert!(err.as_status().is_some());
}

#[test]
fn handles_http_status_failure() {
    let transport = FakeTransport::new(503, "Service Unavailable");
    let ocs = OcsClient::new(transport, FakeAuth);
    let client = UserClient::new(&ocs);

    let err = block_on(client.current_user()).expect_err("503 should fail");
    assert_eq!(err.status_code(), Some(503));
}

#[test]
fn client_display_formatting() {
    let transport = FakeTransport::new(200, CURRENT_USER_ENVELOPE);
    let ocs = OcsClient::new(transport, FakeAuth);
    let client = UserClient::new(&ocs);
    assert_eq!(format!("{client}"), "UserClient");
    assert_eq!(client.ocs().transport().response.status().as_u16(), 200);
}
