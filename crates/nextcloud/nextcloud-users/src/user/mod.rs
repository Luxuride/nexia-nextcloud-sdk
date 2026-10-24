//! User profile and storage quota models.
//!
//! Models user details and quotas returned by Nextcloud user endpoints
//! (`/ocs/v1.php/cloud/user`, `/ocs/v2.php/cloud/user`,
//! `/ocs/v1.php/cloud/users/{userid}`, and `/ocs/v2.php/cloud/users/{userid}`).
//!
//! Official documentation:
//! <https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html>

mod profile;
mod quota;

#[cfg(test)]
mod tests;

pub use profile::User;
pub use quota::Quota;
