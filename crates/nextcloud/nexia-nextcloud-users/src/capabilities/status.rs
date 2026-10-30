use serde::{Deserialize, Serialize};

use crate::wire::lossy_bool;

/// Operational status and runtime state of the Nextcloud instance.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServerStatus {
    /// Whether Nextcloud is installed and configured.
    #[serde(default, deserialize_with = "lossy_bool")]
    pub installed: bool,
    /// Whether the instance is in maintenance mode.
    #[serde(default, deserialize_with = "lossy_bool")]
    pub maintenance: bool,
    /// Whether the database requires a pending migration or upgrade.
    #[serde(
        default,
        alias = "needsDbUpgrade",
        alias = "needs_db_upgrade",
        deserialize_with = "lossy_bool"
    )]
    pub needs_db_upgrade: bool,
    /// Precise version string (e.g. `"30.0.4.1"`).
    #[serde(default)]
    pub version: Option<String>,
    /// Human-readable version string (e.g. `"30.0.4"`).
    #[serde(default, alias = "versionstring", alias = "version_string")]
    pub version_string: Option<String>,
    /// Server edition (e.g. `"Community"`).
    #[serde(default)]
    pub edition: Option<String>,
    /// Product branding name (e.g. `"Nextcloud"`).
    #[serde(default, alias = "productname", alias = "product_name")]
    pub product_name: Option<String>,
}

impl ServerStatus {
    /// Whether the instance is installed and operational.
    pub const fn is_installed(&self) -> bool {
        self.installed
    }

    /// Whether maintenance mode is enabled.
    pub const fn is_maintenance(&self) -> bool {
        self.maintenance
    }

    /// Whether the instance is awaiting a database upgrade.
    pub const fn needs_db_upgrade(&self) -> bool {
        self.needs_db_upgrade
    }

    /// Precise version identifier.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Human-readable version string.
    pub fn version_string(&self) -> Option<&str> {
        self.version_string.as_deref()
    }

    /// Server edition name.
    pub fn edition(&self) -> Option<&str> {
        self.edition.as_deref()
    }

    /// Product brand name.
    pub fn product_name(&self) -> Option<&str> {
        self.product_name.as_deref()
    }
}
