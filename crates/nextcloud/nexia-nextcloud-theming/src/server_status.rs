//! Unauthenticated server status models returned by Nextcloud's `/status.php`.

use serde::{Deserialize, Serialize};

/// Operational status and edition info returned by `GET /status.php`.
///
/// Available without user authentication to allow clients to verify server
/// accessibility, detect maintenance mode, and obtain basic product branding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServerStatusInfo {
    /// Whether the Nextcloud instance is installed and configured.
    #[serde(default)]
    pub installed: bool,
    /// Whether maintenance mode is enabled on the server.
    #[serde(default)]
    pub maintenance: bool,
    /// Whether the instance requires a pending database migration or upgrade.
    #[serde(default, alias = "needsDbUpgrade", alias = "needs_db_upgrade")]
    pub needs_db_upgrade: bool,
    /// Exact server version string (e.g. `"30.0.4.1"`).
    #[serde(default)]
    pub version: Option<String>,
    /// Human-readable server version string (e.g. `"30.0.4"`).
    #[serde(default, alias = "versionstring", alias = "version_string")]
    pub version_string: Option<String>,
    /// Server edition (e.g. `"Community"` or `"Enterprise"`).
    #[serde(default)]
    pub edition: Option<String>,
    /// Product name advertised by the server (e.g. `"Nextcloud"`).
    #[serde(default, alias = "productname", alias = "product_name")]
    pub product_name: Option<String>,
}

impl ServerStatusInfo {
    /// Whether the instance is installed.
    pub const fn is_installed(&self) -> bool {
        self.installed
    }

    /// Whether the instance is in maintenance mode.
    pub const fn is_maintenance(&self) -> bool {
        self.maintenance
    }

    /// Whether the server requires a database upgrade.
    pub const fn needs_db_upgrade(&self) -> bool {
        self.needs_db_upgrade
    }

    /// Canonical product name (e.g. `"Nextcloud"`).
    pub fn product_name(&self) -> Option<&str> {
        self.product_name.as_deref()
    }

    /// Human-readable version string.
    pub fn version_string(&self) -> Option<&str> {
        self.version_string.as_deref()
    }
}
