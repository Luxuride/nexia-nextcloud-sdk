//! Desktop Single-Sign-On discovery engine.
//!
//! Aggregates Nextcloud accounts configured across GNOME Online Accounts,
//! KDE Plasma KAccounts, and official Nextcloud desktop client profiles.

/// Discovered account types and origin classifications.
pub mod account;
/// Discovery coordinator querying desktop subsystems.
pub mod discovery;
/// Error classifications for desktop SSO querying.
pub mod error;

pub use account::{AccountSource, DiscoveredAccount};
pub use discovery::DesktopSso;
pub use error::SsoError;
