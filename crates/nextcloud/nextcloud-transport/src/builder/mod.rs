//! Assembling a transport from a server URL plus client-wide settings.

#[cfg(test)]
mod tests;
mod timeouts;
mod transport_builder;

pub use timeouts::TimeoutConfig;
pub use transport_builder::{DEFAULT_USER_AGENT, TransportBuilder};
