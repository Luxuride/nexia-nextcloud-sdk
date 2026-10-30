use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use nexia_nextcloud_core::{
    AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
};

use super::NotificationClient;

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
        headers.set("Authorization", "Bearer test-token");
    }
}

const LIST_RESPONSE_JSON: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": null
    },
    "data": [
      {
        "notification_id": 61,
        "app": "files_sharing",
        "user": "admin",
        "datetime": "2004-02-12T15:19:21+00:00",
        "object_type": "remote_share",
        "object_id": "13",
        "subject": "You received admin@localhost as a remote share from test",
        "message": "",
        "link": "http://localhost/index.php/apps/files_sharing/pending",
        "actions": [
          {
            "label": "Accept",
            "link": "http://localhost/ocs/v1.php/apps/files_sharing/api/v1/remote_shares/13",
            "type": "POST",
            "primary": true
          },
          {
            "label": "Decline",
            "link": "http://localhost/ocs/v1.php/apps/files_sharing/api/v1/remote_shares/13",
            "type": "DELETE",
            "primary": false
          }
        ]
      }
    ]
  }
}"#;

const SINGLE_NOTIFICATION_JSON: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": null
    },
    "data": {
      "notification_id": 61,
      "app": "files_sharing",
      "user": "admin",
      "datetime": "2004-02-12T15:19:21+00:00",
      "object_type": "remote_share",
      "object_id": "13",
      "subject": "You received admin@localhost as a remote share from test",
      "message": "Detailed message",
      "link": "http://localhost/index.php/apps/files_sharing/pending",
      "actions": [
        {
          "label": "Accept",
          "link": "http://localhost/ocs/v1.php/apps/files_sharing/api/v1/remote_shares/13",
          "type": "POST",
          "primary": true
        }
      ]
    }
  }
}"#;

const SINGLE_IN_ARRAY_JSON: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": null
    },
    "data": [
      {
        "notification_id": 61,
        "app": "files_sharing",
        "user": "admin",
        "datetime": "2004-02-12T15:19:21+00:00",
        "object_type": "remote_share",
        "object_id": "13",
        "subject": "You received admin@localhost as a remote share from test",
        "message": "",
        "link": "",
        "actions": []
      }
    ]
  }
}"#;

const EMPTY_OCS_RESPONSE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": null
    },
    "data": []
  }
}"#;

const FAILURE_OCS_RESPONSE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 997,
      "message": "Notification not found"
    },
    "data": []
  }
}"#;

#[test]
fn list_notifications_parses_json_and_formats_path() {
    let transport = FakeTransport::new(200, LIST_RESPONSE_JSON);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let notifications = block_on(client.list()).unwrap();

    let request = transport.request().expect("request was sent");
    assert_eq!(request.method(), HttpMethod::Get);
    assert_eq!(
        request.path().as_str(),
        "/ocs/v2.php/apps/notifications/api/v2/notifications"
    );
    assert_eq!(request.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(request.headers().get("Accept"), Some("application/json"));
    assert_eq!(
        request.headers().get("Authorization"),
        Some("Bearer test-token")
    );

    assert_eq!(notifications.len(), 1);
    let n = &notifications[0];
    assert_eq!(n.notification_id(), 61);
    assert_eq!(n.app(), "files_sharing");
    assert_eq!(n.user(), "admin");
    assert_eq!(n.datetime(), "2004-02-12T15:19:21+00:00");
    assert_eq!(n.object_type(), "remote_share");
    assert_eq!(n.object_id(), "13");
    assert_eq!(
        n.subject(),
        "You received admin@localhost as a remote share from test"
    );
    assert_eq!(n.message(), "");
    assert_eq!(
        n.link(),
        "http://localhost/index.php/apps/files_sharing/pending"
    );
    assert_eq!(n.actions().len(), 2);

    let action1 = &n.actions()[0];
    assert_eq!(action1.label(), "Accept");
    assert_eq!(
        action1.link(),
        "http://localhost/ocs/v1.php/apps/files_sharing/api/v1/remote_shares/13"
    );
    assert_eq!(action1.method(), "POST");
    assert!(action1.is_primary());

    let action2 = &n.actions()[1];
    assert_eq!(action2.label(), "Decline");
    assert_eq!(
        action2.link(),
        "http://localhost/ocs/v1.php/apps/files_sharing/api/v1/remote_shares/13"
    );
    assert_eq!(action2.method(), "DELETE");
    assert!(!action2.is_primary());
}

#[test]
fn get_notification_parses_single_item_and_formats_path() {
    let transport = FakeTransport::new(200, SINGLE_NOTIFICATION_JSON);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let n = block_on(client.get(61)).unwrap();

    let request = transport.request().expect("request was sent");
    assert_eq!(request.method(), HttpMethod::Get);
    assert_eq!(
        request.path().as_str(),
        "/ocs/v2.php/apps/notifications/api/v2/notifications/61"
    );
    assert_eq!(n.notification_id(), 61);
    assert_eq!(n.app(), "files_sharing");
    assert_eq!(n.message(), "Detailed message");
    assert_eq!(n.actions().len(), 1);
}

#[test]
fn get_notification_accepts_single_item_in_array() {
    let transport = FakeTransport::new(200, SINGLE_IN_ARRAY_JSON);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let n = block_on(client.get(61)).unwrap();
    assert_eq!(n.notification_id(), 61);
}

#[test]
fn delete_notification_formats_path_and_handles_ocs_empty_data() {
    let transport = FakeTransport::new(200, EMPTY_OCS_RESPONSE);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let result = block_on(client.delete(61));
    assert!(result.is_ok());

    let request = transport.request().expect("request was sent");
    assert_eq!(request.method(), HttpMethod::Delete);
    assert_eq!(
        request.path().as_str(),
        "/ocs/v2.php/apps/notifications/api/v2/notifications/61"
    );
    assert_eq!(request.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn delete_all_notifications_formats_path_and_handles_ocs_empty_data() {
    let transport = FakeTransport::new(200, EMPTY_OCS_RESPONSE);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let result = block_on(client.delete_all());
    assert!(result.is_ok());

    let request = transport.request().expect("request was sent");
    assert_eq!(request.method(), HttpMethod::Delete);
    assert_eq!(
        request.path().as_str(),
        "/ocs/v2.php/apps/notifications/api/v2/notifications"
    );
    assert_eq!(request.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn handles_204_no_content_empty_response() {
    let auth = FakeAuth;

    // List on 204 returns empty vector
    let transport_list = FakeTransport::new(204, "");
    let client_list = NotificationClient::new(&transport_list, &auth);
    let list = block_on(client_list.list()).unwrap();
    assert!(list.is_empty());

    // Delete on 204 returns Ok(())
    let transport_del = FakeTransport::new(204, "");
    let client_del = NotificationClient::new(&transport_del, &auth);
    assert!(block_on(client_del.delete(61)).is_ok());

    // Delete all on 204 returns Ok(())
    let transport_del_all = FakeTransport::new(204, "");
    let client_del_all = NotificationClient::new(&transport_del_all, &auth);
    assert!(block_on(client_del_all.delete_all()).is_ok());
}

#[test]
fn handles_200_ok_empty_body() {
    let auth = FakeAuth;

    let transport_list = FakeTransport::new(200, "");
    let client_list = NotificationClient::new(&transport_list, &auth);
    let list = block_on(client_list.list()).unwrap();
    assert!(list.is_empty());

    let transport_del = FakeTransport::new(200, "");
    let client_del = NotificationClient::new(&transport_del, &auth);
    assert!(block_on(client_del.delete(61)).is_ok());
}

#[test]
fn handles_empty_ocs_list() {
    let auth = FakeAuth;
    let transport = FakeTransport::new(200, EMPTY_OCS_RESPONSE);
    let client = NotificationClient::new(&transport, &auth);

    let list = block_on(client.list()).unwrap();
    assert!(list.is_empty());
}

#[test]
fn reports_error_on_ocs_failure_verdict() {
    let auth = FakeAuth;
    let transport = FakeTransport::new(200, FAILURE_OCS_RESPONSE);
    let client = NotificationClient::new(&transport, &auth);

    let err = block_on(client.get(999)).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status(), StatusCode::new(200));
            assert!(status_err.body().contains("Notification not found"));
        }
        other => panic!("expected Error::Status, got {other:?}"),
    }
}

#[test]
fn reports_error_on_http_status_failure() {
    let auth = FakeAuth;
    let transport = FakeTransport::new(404, "Not Found");
    let client = NotificationClient::new(&transport, &auth);

    let err = block_on(client.get(42)).unwrap_err();
    match err {
        Error::Status(status_err) => {
            assert_eq!(status_err.status(), StatusCode::new(404));
        }
        other => panic!("expected Error::Status, got {other:?}"),
    }
}

#[test]
fn reports_decode_error_on_malformed_json() {
    let auth = FakeAuth;
    let transport = FakeTransport::new(200, "{ invalid json");
    let client = NotificationClient::new(&transport, &auth);

    let err = block_on(client.list()).unwrap_err();
    assert!(matches!(err, Error::Decode(_)));
}

#[test]
fn accessors_expose_transport_and_auth() {
    let transport = FakeTransport::new(200, EMPTY_OCS_RESPONSE);
    let auth = FakeAuth;
    let client = NotificationClient::new(&transport, &auth);

    let _t = client.transport();
    let _a = client.auth();
}
