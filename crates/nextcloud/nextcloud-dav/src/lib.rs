//! WebDAV client foundation for the Nextcloud SDK.
//!
//! The crate covers the part of WebDAV Nextcloud's file API leans on first:
//! addressing a path below a user's file tree ([`DavPath`]), describing what
//! to ask for ([`DavProperty`], [`PropfindBody`]), and turning a `207
//! Multi-Status` answer into typed resources ([`DavResource`]) through
//! [`DavClient`].
//!
//! Requests travel through `sdk_core`'s transport seam, so this crate
//! is generic over *how* bytes move and *how* credentials are applied; it
//! never opens a socket itself.
//!
//! XML is written and read by hand. The propfind body is small and fixed,
//! and the reader only has to understand a multistatus envelope, so a full
//! XML implementation would be more weight than value. [`xml`] documents
//! the escaping helpers and the limits of that reader.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
#[cfg(test)]
mod fixtures;
mod method;
mod path;
mod propfind;
mod resource;
pub mod xml;

pub use client::DavClient;
pub use method::{DEPTH_HEADER, Depth};
pub use path::DavPath;
pub use propfind::{DavNamespace, DavProperty, PropfindBody};
pub use resource::DavResource;
