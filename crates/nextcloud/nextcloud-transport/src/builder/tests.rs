use std::time::Duration;

use sdk_core::ServerUrl;

use crate::builder::{DEFAULT_USER_AGENT, TimeoutConfig, TransportBuilder};
use crate::error::TransportBuildError;

fn server() -> ServerUrl {
    ServerUrl::parse("https://cloud.example.org").expect("valid url")
}

#[test]
fn build_requires_a_server_url() {
    let error = TransportBuilder::new()
        .build()
        .expect_err("missing server must fail");
    assert!(matches!(error, TransportBuildError::MissingServerUrl));
}

#[test]
fn build_accepts_a_server_url() {
    let transport = TransportBuilder::with_server(server())
        .build()
        .expect("default config must build");
    assert_eq!(transport.server().as_str(), "https://cloud.example.org");
}

#[test]
fn build_rejects_a_zero_timeout() {
    let error = TransportBuilder::with_server(server())
        .timeouts(TimeoutConfig::none().with_request(Duration::ZERO))
        .build()
        .expect_err("zero timeout must fail");
    assert!(matches!(error, TransportBuildError::ZeroTimeout(_)));
}

#[test]
fn build_rejects_a_header_breaking_user_agent() {
    let error = TransportBuilder::with_server(server())
        .user_agent("bad\nagent")
        .build()
        .expect_err("newline must be rejected");
    assert!(matches!(
        error,
        TransportBuildError::InvalidUserAgent { .. }
    ));
}

#[test]
fn user_agent_override_replaces_the_default() {
    let builder = TransportBuilder::with_server(server()).user_agent("my-app/1.0");
    assert!(DEFAULT_USER_AGENT.starts_with("nextcloud-rust-sdk/"));
    assert!(builder.build().is_ok());
}
