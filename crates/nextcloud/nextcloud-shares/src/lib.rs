//! Nextcloud Files Sharing API client and domain types.
//!
//! Nextcloud manages file and folder sharing through its OCS Files Sharing API
//! at `/ocs/v2.php/apps/files_sharing/api/v1/shares`. This crate provides typed
//! domain models, permissions bitflags, request builders, and an asynchronous
//! client for interacting with shares.
//!
//! - [`ShareClient`] — creates, retrieves, lists and deletes shares.
//! - [`Share`] — a Nextcloud share entity.
//! - [`ShareType`] — the recipient category (user, group, public link, email, etc.).
//! - [`SharePermissions`] — bitflag permissions (read, update, create, delete, share).
//! - [`CreateShare`] — request builder for creating a new share.
//!
//! See the [Nextcloud OCS Share API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html).
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod permissions;
mod request;
mod share;
mod share_type;

pub use client::ShareClient;
pub use permissions::SharePermissions;
pub use request::CreateShare;
pub use share::Share;
pub use share_type::{ShareType, UnknownShareTypeError};
