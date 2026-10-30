use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::Quota;
use crate::wire::lossy_bool;

/// Redacted placeholder for sensitive fields in debug representations.
const REDACTED: &str = "<redacted>";

/// Nextcloud user account details.
///
/// Decoded from the `data` field of user endpoints:
/// - Current authenticated user: `/ocs/v1.php/cloud/user` or `/ocs/v2.php/cloud/user`
/// - Provisioned user profile: `/ocs/v1.php/cloud/users/{userid}` or `/ocs/v2.php/cloud/users/{userid}`
///
/// Official documentation:
/// - User Provisioning API: <https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html>
/// - OCS API Overview: <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#user-metadata>
#[derive(Clone, Default, PartialEq, Serialize)]
pub struct User {
    /// Unique username / user identifier.
    pub id: String,
    /// User display name formatted for the user interface.
    pub display_name: String,
    /// Primary configured email address.
    pub email: Option<String>,
    /// Whether the user account is active and enabled.
    pub enabled: bool,
    /// Storage quota metrics and usage numbers.
    pub quota: Quota,
    /// Group memberships assigned to this user.
    pub groups: Vec<String>,
    /// Groups for which this user has delegated administrative rights.
    pub subadmin: Vec<String>,
    /// Server filesystem storage directory path.
    pub storage_location: Option<String>,
    /// Authentication backend powering this account (e.g. `"Database"` or `"LDAP"`).
    pub backend: Option<String>,
}

#[derive(Deserialize)]
struct UserWire {
    #[serde(default)]
    id: String,
    #[serde(default)]
    displayname: Option<String>,
    #[serde(default, rename = "display-name")]
    display_name_hyphen: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default = "default_enabled", deserialize_with = "lossy_bool")]
    enabled: bool,
    #[serde(default)]
    quota: Quota,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    subadmin: Vec<String>,
    #[serde(default)]
    storage_location: Option<String>,
    #[serde(default, rename = "storageLocation")]
    storage_location_camel: Option<String>,
    #[serde(default, rename = "home")]
    home: Option<String>,
    #[serde(default)]
    backend: Option<String>,
}

impl<'de> Deserialize<'de> for User {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = UserWire::deserialize(deserializer)?;
        let display_name = wire
            .displayname
            .or(wire.display_name_hyphen)
            .or(wire.display_name)
            .unwrap_or_default();
        let storage_location = wire
            .storage_location
            .or(wire.storage_location_camel)
            .or(wire.home);
        Ok(Self {
            id: wire.id,
            display_name,
            email: wire.email,
            enabled: wire.enabled,
            quota: wire.quota,
            groups: wire.groups,
            subadmin: wire.subadmin,
            storage_location,
            backend: wire.backend,
        })
    }
}

const fn default_enabled() -> bool {
    true
}

impl User {
    /// Unique user ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// User's display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Primary email address, if set.
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Whether this user account is active.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Storage quota constraints and usage.
    pub const fn quota(&self) -> &Quota {
        &self.quota
    }

    /// Group memberships.
    pub fn groups(&self) -> &[String] {
        &self.groups
    }

    /// Groups for which this user is a subadmin.
    pub fn subadmin(&self) -> &[String] {
        &self.subadmin
    }

    /// Storage location path on the server, if returned.
    pub fn storage_location(&self) -> Option<&str> {
        self.storage_location.as_deref()
    }

    /// Authentication backend type.
    pub fn backend(&self) -> Option<&str> {
        self.backend.as_deref()
    }
}

impl fmt::Debug for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("email", &self.email.as_ref().map(|_| REDACTED))
            .field("enabled", &self.enabled)
            .field("quota", &self.quota)
            .field("groups", &self.groups)
            .field("subadmin", &self.subadmin)
            .field(
                "storage_location",
                &self.storage_location.as_ref().map(|_| REDACTED),
            )
            .field("backend", &self.backend)
            .finish()
    }
}
