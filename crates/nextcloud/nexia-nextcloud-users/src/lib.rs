//! Nextcloud user provisioning and server capabilities client.
//!
//! This crate models the Nextcloud OCS User and Server Capabilities APIs,
//! wrapping [`nexia_nextcloud_ocs::OcsClient`] with strongly typed representations
//! of:
//! - Server capabilities discovery ([`Capabilities`]): Nextcloud version,
//!   operational status, poll intervals, WebDAV root, file chunking, undelete,
//!   versioning, and theming branding.
//! - User details and storage quota ([`User`], [`Quota`]): user ID, display
//!   name, email, account status, group memberships, and storage limits.
//!
//! Sensitive fields (such as user email addresses and internal storage locations)
//! are strictly redacted in [`std::fmt::Debug`] formatting to prevent credential
//! or personal data leakage into log files.
//!
//! Official Nextcloud API Documentation:
//! - [OCS API Overview](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html)
//! - [Capabilities API](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#capabilities-api)
//! - [User Metadata](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#user-metadata)
//! - [User Provisioning API](https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html)
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod capabilities;
mod client;
mod user;
pub(crate) mod wire;

pub use capabilities::{
    Capabilities, CapabilitiesData, ChunkedUpload, CoreCapabilities, FilesCapabilities,
    ServerStatus, ServerVersion, ThemingCapabilities,
};
pub use client::UserClient;
pub use user::{Quota, User};
