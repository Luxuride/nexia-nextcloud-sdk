//! Assembling a transport from a server URL plus client-wide settings.

#[cfg(test)]
mod tests;
mod timeouts;
mod transport_builder;

pub use timeouts::{DEFAULT_CONNECT_TIMEOUT, DEFAULT_REQUEST_TIMEOUT, TimeoutConfig};
pub use transport_builder::{DEFAULT_USER_AGENT, TransportBuilder};
