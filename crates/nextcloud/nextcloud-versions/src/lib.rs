//! Nextcloud WebDAV file versions client.
//!
//! Provides operations for listing revisions of a file, downloading past revision
//! bytes, and restoring an older revision via Nextcloud's WebDAV versions endpoints.
//!
//! # Endpoints
//!
//! - Root path: `/remote.php/dav/versions/{user}/versions/{file_id}`
//! - `PROPFIND` lists previous revisions of a file with their metadata.
//! - `GET` downloads the bytes of a specific revision.
//! - `MOVE` restores a revision to `/remote.php/dav/versions/{user}/restore/{file_id}`.
//!
//! All operations are carried out through [`VersionsClient`].
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod decode;
mod validation;
mod version;

pub use client::VersionsClient;
pub use version::FileVersion;
