//! Nextcloud theming, custom branding metadata, and unauthenticated server status client.
//!
//! This crate provides domain models and an asynchronous client for:
//! - Fetching server-side visual branding from `/ocs/v2.php/apps/theming/api/v1/theme` ([`ThemeInfo`]).
//! - Probing server availability and product metadata before login via unauthenticated `/status.php` ([`ServerStatusInfo`]).
//! - Specifying client-side custom application branding metadata ([`CustomBranding`]).
//! - Merging client overrides with server theme info into unified [`EffectiveBranding`].
//!
//! Official Nextcloud documentation:
//! - [Theming App](https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/theming.html)
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod client;
pub mod custom_branding;
pub mod effective_branding;
pub mod error;
pub mod server_status;
pub mod theme_info;

pub use client::ThemingClient;
pub use custom_branding::CustomBranding;
pub use effective_branding::EffectiveBranding;
pub use error::ThemingError;
pub use server_status::ServerStatusInfo;
pub use theme_info::ThemeInfo;
