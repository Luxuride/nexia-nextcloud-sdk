//! Errors raised while configuring the transport or preparing a request.

mod build;
mod header;

pub use build::TransportBuildError;
pub(crate) use header::HeaderCopyError;
