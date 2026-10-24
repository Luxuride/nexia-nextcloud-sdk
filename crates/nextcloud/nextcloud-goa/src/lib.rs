//! GNOME Online Accounts integration for Nextcloud.
//!
//! This crate provides direct D-Bus integration with GNOME Online Accounts
//! (`org.gnome.OnlineAccounts`) using [`zbus`] to securely discover and
//! extract Nextcloud/ownCloud accounts configured on the Linux desktop session.
//!
//! # Architecture
//!
//! - [`GnomeOnlineAccounts`]: Connects to session D-Bus and queries the object manager.
//! - [`GoaAccount`]: Represents a discovered account with base URL and assembled [`BasicAuth`](nextcloud_auth::BasicAuth).
//! - [`proxy`]: Strongly-typed `zbus` proxies for GOA interfaces.
//! - [`GoaError`]: Typed error enum for D-Bus communication and configuration errors.
//! - [`parse_webdav_url`]: Normalises WebDAV URIs into Nextcloud base URLs.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod account;
mod client;
mod error;
pub mod proxy;
mod url;

pub use account::GoaAccount;
pub use client::GnomeOnlineAccounts;
pub use error::GoaError;
pub use url::parse_webdav_url;
