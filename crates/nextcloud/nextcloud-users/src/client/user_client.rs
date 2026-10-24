use nextcloud_ocs::{OcsClient, OcsFormat, Query};
use sdk_core::{ApiPath, AuthProvider, Error, Transport};

use crate::capabilities::Capabilities;
use crate::user::User;

/// Client for Nextcloud user provisioning and server capabilities.
///
/// Wraps an [`OcsClient`] by reference and provides high-level typed calls for:
/// - Server capabilities discovery (`/ocs/v1.php/cloud/capabilities` or `/ocs/v2.php/cloud/capabilities`)
/// - Current authenticated user metadata (`/ocs/v1.php/cloud/user` or `/ocs/v2.php/cloud/user`)
/// - User profile lookup by user ID (`/ocs/v1.php/cloud/users/{userid}` or `/ocs/v2.php/cloud/users/{userid}`)
///
/// Official documentation:
/// - Nextcloud OCS Overview: <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html>
/// - User Metadata: <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#user-metadata>
/// - Capabilities API: <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#capabilities-api>
/// - User Provisioning API: <https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html>
#[derive(Clone, Copy, Debug)]
pub struct UserClient<'a, T: Transport, A: AuthProvider> {
    ocs: &'a OcsClient<T, A>,
}

impl<'a, T: Transport, A: AuthProvider> UserClient<'a, T, A> {
    /// Binds a user client to an underlying [`OcsClient`].
    pub const fn new(ocs: &'a OcsClient<T, A>) -> Self {
        Self { ocs }
    }

    /// Borrows the underlying [`OcsClient`].
    pub const fn ocs(&self) -> &'a OcsClient<T, A> {
        self.ocs
    }

    /// Fetches server capabilities.
    ///
    /// Sends a `GET` request to `/ocs/v2.php/cloud/capabilities?format=json`
    /// (the v2 equivalent of `/ocs/v1.php/cloud/capabilities`).
    ///
    /// See <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#capabilities-api>.
    pub async fn capabilities(&self) -> Result<Capabilities, Error> {
        let path = ApiPath::ocs("cloud/capabilities")?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Fetches the profile and storage quota of the currently authenticated user.
    ///
    /// Sends a `GET` request to `/ocs/v2.php/cloud/user?format=json`
    /// (the v2 equivalent of `/ocs/v1.php/cloud/user`).
    ///
    /// See <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html>.
    pub async fn current_user(&self) -> Result<User, Error> {
        let path = ApiPath::ocs("cloud/user")?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Fetches the profile and storage quota of a user by their user identifier.
    ///
    /// Sends a `GET` request to `/ocs/v2.php/cloud/users/{userid}?format=json`
    /// (the v2 equivalent of `/ocs/v1.php/cloud/users/{userid}`).
    ///
    /// See <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#user-metadata>.
    pub async fn get_user(&self, user_id: &str) -> Result<User, Error> {
        let path = ApiPath::ocs(format!("cloud/users/{user_id}"))?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }
}

impl<T: Transport, A: AuthProvider> std::fmt::Display for UserClient<'_, T, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UserClient")
    }
}
