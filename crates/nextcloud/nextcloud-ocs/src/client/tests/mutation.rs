use std::future::IntoFuture;

use sdk_core::{ApiPath, HttpMethod};
use serde::{Deserialize, Serialize};

use super::{FakeTransport, block_on, client};
use crate::query::Query;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct UserRequest {
    userid: String,
    email: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct UserResponse {
    id: String,
    enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShareRequest {
    path: String,
    share_type: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct ShareResponse {
    id: u64,
    token: String,
}

const USER_POST_OK: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 100,
      "message": "OK"
    },
    "data": {
      "id": "alice",
      "enabled": true
    }
  }
}"#;

const SHARE_PUT_OK: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "id": 42,
      "token": "k8sdf0"
    }
  }
}"#;

const SHARE_DELETE_OK: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 100,
      "message": "OK"
    },
    "data": {
      "id": 42,
      "token": "deleted"
    }
  }
}"#;

const OCS_STATUS_101_FAILURE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 101,
      "message": "User already exists"
    },
    "data": []
  }
}"#;

const OCS_STATUS_997_FAILURE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 997,
      "message": "Current user is not allowed to alter user"
    },
    "data": []
  }
}"#;

fn users_path() -> ApiPath {
    ApiPath::ocs("cloud/users").expect("valid OCS path")
}

fn user_alice_path() -> ApiPath {
    ApiPath::ocs("cloud/users/alice").expect("valid OCS path")
}

fn share_path() -> ApiPath {
    ApiPath::ocs("apps/files_sharing/api/v1/shares/42").expect("valid OCS path")
}

#[test]
fn post_sends_json_and_unwraps_payload() {
    let transport = FakeTransport::new(200, USER_POST_OK);
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
    };

    let user: UserResponse = block_on(
        client(transport.clone())
            .post(users_path(), &request_body)
            .into_future(),
    )
    .expect("post unwraps response data");

    assert_eq!(user.id, "alice");
    assert!(user.enabled);

    let recorded = transport.request().expect("transport received request");
    assert_eq!(recorded.method(), HttpMethod::Post);
    assert_eq!(recorded.headers().get("Accept"), Some("application/json"));
    assert_eq!(
        recorded.headers().get("Content-Type"),
        Some("application/json")
    );
    assert_eq!(recorded.headers().get("OCS-APIRequest"), Some("true"));
    assert_eq!(
        recorded.headers().get("Authorization"),
        Some("Bearer test-token")
    );

    let body_str = std::str::from_utf8(recorded.body().expect("body is present")).unwrap();
    assert!(body_str.contains("\"userid\":\"alice\""));
    assert!(body_str.contains("\"email\":\"alice@example.com\""));
}

#[test]
fn put_sends_json_and_unwraps_payload() {
    let transport = FakeTransport::new(200, SHARE_PUT_OK);
    let request_body = ShareRequest {
        path: "/Documents".to_owned(),
        share_type: 3,
    };

    let share: ShareResponse = block_on(
        client(transport.clone())
            .put(share_path(), &request_body)
            .into_future(),
    )
    .expect("put unwraps response data");

    assert_eq!(share.id, 42);
    assert_eq!(share.token, "k8sdf0");

    let recorded = transport.request().expect("transport received request");
    assert_eq!(recorded.method(), HttpMethod::Put);
    assert_eq!(recorded.headers().get("Accept"), Some("application/json"));
    assert_eq!(
        recorded.headers().get("Content-Type"),
        Some("application/json")
    );
    assert_eq!(recorded.headers().get("OCS-APIRequest"), Some("true"));
}

#[test]
fn delete_sends_no_body_and_unwraps_payload() {
    let transport = FakeTransport::new(200, SHARE_DELETE_OK);

    let share: ShareResponse =
        block_on(client(transport.clone()).delete(share_path()).into_future())
            .expect("delete unwraps response data");

    assert_eq!(share.id, 42);

    let recorded = transport.request().expect("transport received request");
    assert_eq!(recorded.method(), HttpMethod::Delete);
    assert_eq!(recorded.headers().get("Accept"), Some("application/json"));
    assert_eq!(recorded.headers().get("OCS-APIRequest"), Some("true"));
    assert!(
        recorded.headers().get("Content-Type").is_none(),
        "DELETE without body does not specify Content-Type"
    );
    assert!(recorded.body().is_none(), "DELETE has no body");
}

#[test]
fn post_statuscode_101_becomes_status_error() {
    let transport = FakeTransport::new(200, OCS_STATUS_101_FAILURE);
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
    };

    let error = block_on(
        client(transport)
            .post::<UserRequest, UserResponse>(users_path(), &request_body)
            .into_future(),
    )
    .expect_err("OCS statuscode 101 must fail");

    let status = error.as_status().expect("mapped to status error");
    assert_eq!(status.method(), "POST");
    assert_eq!(error.status_code(), Some(200));
    assert!(
        error.body().unwrap_or_default().contains("101"),
        "body preserves OCS 101 statuscode"
    );
}

#[test]
fn put_statuscode_997_becomes_status_error() {
    let transport = FakeTransport::new(200, OCS_STATUS_997_FAILURE);
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "new@example.com".to_owned(),
    };

    let error = block_on(
        client(transport)
            .put::<UserRequest, UserResponse>(user_alice_path(), &request_body)
            .into_future(),
    )
    .expect_err("OCS statuscode 997 must fail");

    let status = error.as_status().expect("mapped to status error");
    assert_eq!(status.method(), "PUT");
    assert!(
        error.body().unwrap_or_default().contains("997"),
        "body preserves OCS 997 statuscode"
    );
}

#[test]
fn delete_failing_statuscode_becomes_status_error() {
    let transport = FakeTransport::new(200, OCS_STATUS_101_FAILURE);

    let error = block_on(
        client(transport)
            .delete::<ShareResponse>(share_path())
            .into_future(),
    )
    .expect_err("OCS failure status must fail");

    let status = error.as_status().expect("mapped to status error");
    assert_eq!(status.method(), "DELETE");
}

#[test]
fn mutation_with_query_reaches_path() {
    let transport = FakeTransport::new(200, USER_POST_OK);
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
    };

    let query = Query::new().param("format", "json");
    let _: UserResponse = block_on(
        client(transport.clone())
            .post_with_query(users_path(), query, &request_body)
            .into_future(),
    )
    .expect("succeeds");

    assert_eq!(
        transport
            .request()
            .map(|req| req.path().as_str().to_owned()),
        Some("/ocs/v2.php/cloud/users?format=json".to_owned())
    );
}

#[test]
fn mutation_rejects_non_ocs_path() {
    let transport = FakeTransport::new(200, USER_POST_OK);
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
    };

    let dav_path = ApiPath::dav("files/alice").expect("valid DAV path");
    let error = block_on(
        client(transport)
            .post::<UserRequest, UserResponse>(dav_path, &request_body)
            .into_future(),
    )
    .expect_err("DAV path rejected");

    assert!(error.as_url().is_some());
}

#[test]
fn http_error_becomes_status_error_naming_method() {
    let transport = FakeTransport::new(500, "Internal Server Error");
    let request_body = UserRequest {
        userid: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
    };

    let error = block_on(
        client(transport)
            .post::<UserRequest, UserResponse>(users_path(), &request_body)
            .into_future(),
    )
    .expect_err("HTTP 500 must fail");

    let status = error.as_status().expect("status error");
    assert_eq!(status.method(), "POST");
    assert_eq!(status.status().as_u16(), 500);
}
