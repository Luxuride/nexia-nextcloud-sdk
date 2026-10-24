//! Nextcloud desktop client configuration parser and account discovery.
//!
//! Discovers and parses configured accounts from the official Nextcloud desktop client
//! across Linux, macOS, and Windows. Enables instant zero-login integration for applications.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Discovered account representation and authentication types.
pub mod account;
/// Credentials lookup helpers via `nextcloud-sdk-keyring`.
#[cfg(feature = "keyring")]
pub mod credentials;
/// Error definitions for configuration parsing and discovery.
pub mod error;
/// INI and QSettings parser for `nextcloud.cfg`.
pub mod parser;
/// Standard configuration file path resolution across operating systems.
pub mod paths;

pub use account::{AuthType, DesktopClientAccount};
#[cfg(feature = "keyring")]
pub use credentials::find_account_credentials;
pub use error::ClientConfigError;
pub use parser::ClientConfigParser;
pub use paths::{discover_config_paths, find_active_config_path};
