use nextcloud_sdk_core::ServerUrl;

/// An online account discovered from KDE Plasma's KAccounts / Accounts-SSO system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KAccount {
    /// Numerical account identifier in KAccounts.
    pub id: u32,
    /// Human-friendly display label of the account.
    pub name: String,
    /// Provider identifier (e.g. `"nextcloud"` or `"owncloud"`).
    pub provider: String,
    /// Target Nextcloud server URL.
    pub server_url: ServerUrl,
    /// Configured account username.
    pub username: String,
    /// Whether the account is currently enabled in KDE Plasma settings.
    pub enabled: bool,
    /// Enabled services (e.g. `["nextcloud-storage", "nextcloud-caldav"]`).
    pub services: Vec<String>,
}

impl KAccount {
    /// Candidate keyring account keys used by Accounts-SSO / KAccounts / signond.
    pub fn candidate_keychain_keys(&self) -> Vec<String> {
        let base_url = self.server_url.as_str().trim_end_matches('/');
        let user = &self.username;
        let id_str = self.id.to_string();

        vec![
            format!("kaccounts:{id_str}"),
            format!("signond:{id_str}"),
            format!("{base_url}:{user}"),
            format!("{user}@{base_url}"),
            id_str,
        ]
    }
}
