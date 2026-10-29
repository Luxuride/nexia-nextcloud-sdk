use std::future::IntoFuture;

use nexia_nextcloud_core::ApiPath;

use super::{
    CAPABILITIES_OK, Capabilities, FakeTransport, OCS_FAILURE, block_on, capabilities_path, client,
};
use crate::query::{OcsFormat, Query};

#[test]
fn ok_response_yields_the_payload() {
    let transport = FakeTransport::new(200, CAPABILITIES_OK);
    let capabilities: Capabilities = block_on(
        client(transport.clone())
            .get(capabilities_path(), Query::new())
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
            .get(capabilities_path(), Query::new())
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
            .get(capabilities_path(), Query::new().format(OcsFormat::Json))
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
            .get::<Capabilities>(capabilities_path(), Query::new())
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
            .get::<Capabilities>(capabilities_path(), Query::new())
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
            .get::<Capabilities>(capabilities_path(), Query::new())
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
            .get::<Capabilities>(capabilities_path(), Query::new())
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
