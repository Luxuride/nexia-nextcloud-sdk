use std::future::IntoFuture;

use nexia_nextcloud_core::ApiPath;
use serde::{Serialize, Serializer};

use super::{FakeTransport, block_on, client};

struct FailingPayload;

impl Serialize for FailingPayload {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom("forced serialization failure"))
    }
}

fn path() -> ApiPath {
    ApiPath::ocs("cloud/users").expect("valid OCS path")
}

#[test]
fn post_serialization_failure_aborts_before_sending() {
    let transport = FakeTransport::new(200, "{}");
    let error = block_on(
        client(transport.clone())
            .post::<FailingPayload, ()>(path(), &FailingPayload)
            .into_future(),
    )
    .expect_err("serialization failure must return error");

    let decode = error.as_decode().expect("maps to decode error variant");
    assert!(
        decode.target().contains("FailingPayload"),
        "target type is named in decode error"
    );
    assert!(
        error.to_string().contains("forced serialization failure"),
        "source cause is preserved"
    );
    assert!(
        !error.is_retryable(),
        "serialization error is not retryable"
    );
    assert!(
        transport.request().is_none(),
        "no request is sent when serialization fails"
    );
}

#[test]
fn put_serialization_failure_aborts_before_sending() {
    let transport = FakeTransport::new(200, "{}");
    let error = block_on(
        client(transport.clone())
            .put::<FailingPayload, ()>(path(), &FailingPayload)
            .into_future(),
    )
    .expect_err("serialization failure must return error");

    let decode = error.as_decode().expect("maps to decode error variant");
    assert!(decode.target().contains("FailingPayload"));
    assert!(
        error.to_string().contains("forced serialization failure"),
        "source cause is preserved"
    );
    assert!(!error.is_retryable());
    assert!(transport.request().is_none());
}
