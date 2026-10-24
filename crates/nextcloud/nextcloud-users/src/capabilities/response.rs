use serde::{Deserialize, Serialize};

use super::{CoreCapabilities, FilesCapabilities, ServerVersion, ThemingCapabilities};

/// Inner capabilities map grouping feature sets by subsystem.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CapabilitiesData {
    /// Core server capabilities such as status, poll interval, and WebDAV root.
    #[serde(default)]
    pub core: CoreCapabilities,
    /// Files application capabilities such as chunking, undelete, and versioning.
    #[serde(default)]
    pub files: FilesCapabilities,
    /// Theming and visual branding customization settings, if enabled.
    #[serde(default)]
    pub theming: Option<ThemingCapabilities>,
}

/// Server capabilities response payload returned by Nextcloud.
///
/// Decoded from the `data` field of the OCS response from
/// `/ocs/v1.php/cloud/capabilities` or `/ocs/v2.php/cloud/capabilities`.
///
/// See the [Nextcloud Capabilities API Documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#capabilities-api).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Capabilities {
    /// Nextcloud release version block, if provided at top-level.
    #[serde(default)]
    pub version: Option<ServerVersion>,
    /// Capabilities map categorized by functional area.
    #[serde(default)]
    pub capabilities: CapabilitiesData,
}

impl Capabilities {
    /// Version information reported by the server, if present.
    pub fn version(&self) -> Option<&ServerVersion> {
        self.version.as_ref()
    }

    /// Core server capabilities.
    pub const fn core(&self) -> &CoreCapabilities {
        &self.capabilities.core
    }

    /// Files app capabilities.
    pub const fn files(&self) -> &FilesCapabilities {
        &self.capabilities.files
    }

    /// Theming and branding capabilities, if enabled.
    pub fn theming(&self) -> Option<&ThemingCapabilities> {
        self.capabilities.theming.as_ref()
    }

    /// Canonical version string, inspecting the top-level `version` block first
    /// and falling back to `core.status.version_string`.
    pub fn server_version(&self) -> Option<&str> {
        self.version
            .as_ref()
            .map(|v| v.string.as_str())
            .or_else(|| {
                self.capabilities
                    .core
                    .status
                    .as_ref()
                    .and_then(|s| s.version_string.as_deref())
            })
    }
}
