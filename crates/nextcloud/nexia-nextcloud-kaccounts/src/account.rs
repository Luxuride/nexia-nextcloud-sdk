use nexia_nextcloud_core::ServerUrl;

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
    ///
    /// A bare account id is deliberately not a candidate: it is shared by every service of that
    /// account, so a lookup could return a token minted for a different service.
    pub fn candidate_keychain_keys(&self) -> Vec<String> {
        let base_url = self.server_url.as_str().trim_end_matches('/');
        let user = &self.username;
        let id_str = self.id.to_string();

        vec![
            format!("kaccounts:{id_str}"),
            format!("signond:{id_str}"),
            format!("{base_url}:{user}"),
            format!("{user}@{base_url}"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> KAccount {
        KAccount {
            id: 42,
            name: "KDE Cloud".to_string(),
            provider: "nextcloud".to_string(),
            server_url: ServerUrl::parse("https://cloud.example.com").unwrap(),
            username: "alice".to_string(),
            enabled: true,
            services: Vec::new(),
        }
    }

    #[test]
    fn every_candidate_names_the_account() {
        let keys = account().candidate_keychain_keys();

        assert_eq!(
            keys,
            vec![
                "kaccounts:42".to_string(),
                "signond:42".to_string(),
                "https://cloud.example.com:alice".to_string(),
                "alice@https://cloud.example.com".to_string(),
            ]
        );
        // A bare account id is shared by every service of that account.
        assert!(!keys.contains(&"42".to_string()));
    }
}
