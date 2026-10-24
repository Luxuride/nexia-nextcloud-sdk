//! HTTP transport for the Nextcloud SDK.
//!
//! The crate fills the [`sdk_core::Transport`] seam with a `reqwest`
//! backed implementation: a [`TransportBuilder`] turns a
//! [`ServerUrl`](sdk_core::ServerUrl) plus timeout and user-agent
//! settings into a [`ReqwestTransport`], which resolves API paths to absolute
//! URLs and hands raw responses back to the API crates unchanged.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod builder;
mod error;
mod header_copy;
mod method;
mod reqwest_transport;
mod response_convert;

pub use builder::{DEFAULT_USER_AGENT, TimeoutConfig, TransportBuilder};
pub use error::TransportBuildError;
pub use reqwest_transport::ReqwestTransport;
