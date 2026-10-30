use std::future::IntoFuture;

use nexia_nextcloud_core::ApiPath;

use super::{FakeTransport, block_on, client};

const EMPTY_ARRAY_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 100,
      "message": "OK"
    },
    "data": []
  }
}"#;

const EMPTY_OBJECT_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {}
  }
}"#;

const NO_DATA_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 100,
      "message": "OK"
    }
  }
}"#;

const NON_EMPTY_DATA_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "ok",
      "statuscode": 200,
      "message": "OK"
    },
    "data": {
      "ignored_field": "some_value"
    }
  }
}"#;

const FAILURE_ENVELOPE: &str = r#"{
  "ocs": {
    "meta": {
      "status": "failure",
      "statuscode": 101,
      "message": "User not found"
    },
    "data": []
  }
}"#;

fn delete_path() -> ApiPath {
    ApiPath::ocs("cloud/users/bob").expect("valid OCS path")
}

#[test]
fn delete_accepts_unit_type_on_empty_data_array() {
    let transport = FakeTransport::new(200, EMPTY_ARRAY_ENVELOPE);
    let result: () = block_on(client(transport).delete::<()>(delete_path()).into_future())
        .expect("unit response succeeds with empty array data");

    assert_eq!(result, ());
}

#[test]
fn post_accepts_unit_type_on_empty_data_object() {
    let transport = FakeTransport::new(200, EMPTY_OBJECT_ENVELOPE);
    let result: () = block_on(
        client(transport)
            .post::<_, ()>(delete_path(), &"sample body")
            .into_future(),
    )
    .expect("unit response succeeds with empty object data");

    assert_eq!(result, ());
}

#[test]
fn put_accepts_unit_type_when_data_is_omitted() {
    let transport = FakeTransport::new(200, NO_DATA_ENVELOPE);
    let result: () = block_on(
        client(transport)
            .put::<_, ()>(delete_path(), &"sample body")
            .into_future(),
    )
    .expect("unit response succeeds when data is omitted");

    assert_eq!(result, ());
}

#[test]
fn unit_type_succeeds_when_caller_only_cares_about_verdict_success() {
    let transport = FakeTransport::new(200, NON_EMPTY_DATA_ENVELOPE);
    let result: () = block_on(client(transport).delete::<()>(delete_path()).into_future())
        .expect("unit response succeeds on verdict success even if payload is present");

    assert_eq!(result, ());
}

#[test]
fn unit_type_still_fails_when_verdict_is_failure() {
    let transport = FakeTransport::new(200, FAILURE_ENVELOPE);
    let error = block_on(client(transport).delete::<()>(delete_path()).into_future())
        .expect_err("verdict failure must still fail for unit type");

    assert!(error.as_status().is_some());
    assert_eq!(error.as_status().unwrap().method(), "DELETE");
}

#[test]
fn unit_type_fails_on_http_error() {
    let transport = FakeTransport::new(404, "Not Found");
    let error = block_on(client(transport).delete::<()>(delete_path()).into_future())
        .expect_err("HTTP 404 must fail for unit type");

    assert!(error.as_status().is_some());
    assert_eq!(error.status_code(), Some(404));
}
