//! Contract checks for the public surface: the transport must be shareable
//! across tasks and must not describe credentials anywhere.

use nexia_nextcloud_core::{ServerUrl, Transport};
use nexia_nextcloud_transport::{ReqwestTransport, TimeoutConfig, TransportBuilder};

fn assert_send_sync<T: Send + Sync>() {}

fn accepts_transport<T: Transport>(transport: &T) -> &T {
    transport
}

fn build() -> ReqwestTransport {
    let server = ServerUrl::parse("https://cloud.example.org").expect("valid url");
    TransportBuilder::with_server(server)
        .user_agent("nexia-nextcloud-sdk-tests/1.0")
        .timeouts(TimeoutConfig::new())
        .build()
        .expect("default configuration must build")
}

#[test]
fn transport_is_shareable_and_implements_the_trait() {
    assert_send_sync::<ReqwestTransport>();
    let transport = build();
    assert_eq!(
        accepts_transport(&transport).server().as_str(),
        "https://cloud.example.org"
    );
}

#[test]
fn debug_describes_the_server_only() {
    let rendered = format!("{:?}", build());
    assert!(rendered.contains("cloud.example.org"));
    assert!(rendered.contains("ReqwestTransport"));
    assert!(!rendered.to_lowercase().contains("authorization"));
}

#[test]
fn builder_exposes_its_configuration() {
    let server = ServerUrl::parse("https://cloud.example.org:8443/nextcloud").expect("valid url");
    let builder = TransportBuilder::with_server(server).timeouts(TimeoutConfig::none());
    assert_eq!(
        builder.server_url().map(ServerUrl::as_str),
        Some("https://cloud.example.org:8443/nextcloud")
    );
    assert_eq!(builder.configured_timeouts().request(), None);
}
