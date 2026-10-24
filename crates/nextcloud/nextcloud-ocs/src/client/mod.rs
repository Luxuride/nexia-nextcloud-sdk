//! The OCS client: build a request, unwrap the envelope, return the payload.

mod ocs_client;
mod path;

pub use ocs_client::OcsClient;

#[cfg(test)]
mod tests;
