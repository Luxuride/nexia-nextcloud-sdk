use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use nextcloud_sdk_core::ServerUrl;

use crate::account::{AuthType, DesktopClientAccount};
use crate::error::ClientConfigError;

/// Parser for Nextcloud desktop client configuration files (`nextcloud.cfg`).
#[derive(Clone, Debug, Default)]
pub struct ClientConfigParser;

impl ClientConfigParser {
    /// Creates a new configuration parser.
    pub fn new() -> Self {
        Self
    }

    /// Reads and parses desktop accounts from a file path.
    pub fn parse_file(path: &Path) -> Result<Vec<DesktopClientAccount>, ClientConfigError> {
        tracing::debug!(path = %path.display(), "Parsing Nextcloud desktop client config file");
        let content = fs::read_to_string(path)
            .map_err(|err| ClientConfigError::io(path.to_path_buf(), err))?;
        Self::parse_str(&content)
    }

    /// Parses desktop accounts from an INI configuration string.
    pub fn parse_str(content: &str) -> Result<Vec<DesktopClientAccount>, ClientConfigError> {
        let mut accounts_map: BTreeMap<usize, BTreeMap<String, String>> = BTreeMap::new();
        let mut current_section = String::new();

        for raw_line in content.lines() {
            let line = raw_line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Section headers: [Accounts] or [Account_0]
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim().to_string();
                continue;
            }

            // Key-value pairs
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();

            if current_section.eq_ignore_ascii_case("Accounts") {
                // Format: <index>\<attribute> or <index>/<attribute>
                if let Some((idx_str, attr)) = key.split_once('\\').or_else(|| key.split_once('/'))
                    && let Ok(idx) = idx_str.parse::<usize>()
                {
                    accounts_map
                        .entry(idx)
                        .or_default()
                        .insert(attr.to_string(), val.to_string());
                }
            } else if let Some(suffix) = current_section.strip_prefix("Account_") {
                // Legacy owncloud format: [Account_0]
                if let Ok(idx) = suffix.parse::<usize>() {
                    accounts_map
                        .entry(idx)
                        .or_default()
                        .insert(key.to_string(), val.to_string());
                }
            }
        }

        let mut accounts = Vec::new();
        for (idx, attrs) in accounts_map {
            let Some(raw_url) = attrs.get("url").cloned() else {
                continue;
            };
            let Some(username) = attrs
                .get("user")
                .or_else(|| attrs.get("dav_user"))
                .or_else(|| attrs.get("webflow_user"))
                .cloned()
            else {
                continue;
            };

            let server_url = ServerUrl::parse(&raw_url)
                .map_err(|err| ClientConfigError::invalid_url(raw_url, err))?;

            let auth_type = attrs
                .get("authType")
                .map(|s| AuthType::from_str_lossy(s))
                .unwrap_or_default();

            accounts.push(DesktopClientAccount {
                index: idx,
                server_url,
                username,
                dav_user: attrs.get("dav_user").cloned(),
                display_name: attrs.get("displayName").cloned(),
                auth_type,
                server_version: attrs.get("serverVersion").cloned(),
                webflow_user: attrs.get("webflow_user").cloned(),
            });
        }

        tracing::info!(count = accounts.len(), "Parsed desktop client accounts");
        Ok(accounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_NEXTCLOUD_CFG: &str = r#"
[General]
clientVersion=3.13.0
confirmExternalStorage=true

[Accounts]
version=2
0\authType=webflow
0\dav_user=admin
0\displayName=Administrator
0\serverVersion=29.0.4.1
0\url=https://cloud.example.com
0\user=admin
0\webflow_user=admin

1\authType=http
1\url=https://neurocloud.luxuride.eu
1\user=alice
1\displayName=Alice Dev
"#;

    #[test]
    fn parses_multiple_accounts_successfully() {
        let accounts = ClientConfigParser::parse_str(SAMPLE_NEXTCLOUD_CFG).unwrap();
        assert_eq!(accounts.len(), 2);

        let acc0 = &accounts[0];
        assert_eq!(acc0.index, 0);
        assert_eq!(acc0.server_url.as_str(), "https://cloud.example.com");
        assert_eq!(acc0.username, "admin");
        assert_eq!(acc0.dav_username(), "admin");
        assert_eq!(acc0.auth_type, AuthType::Webflow);
        assert_eq!(acc0.display_name.as_deref(), Some("Administrator"));
        assert_eq!(acc0.server_version.as_deref(), Some("29.0.4.1"));

        let acc1 = &accounts[1];
        assert_eq!(acc1.index, 1);
        assert_eq!(acc1.server_url.as_str(), "https://neurocloud.luxuride.eu");
        assert_eq!(acc1.username, "alice");
        assert_eq!(acc1.auth_type, AuthType::Http);
        assert_eq!(acc1.display_name.as_deref(), Some("Alice Dev"));
    }

    #[test]
    fn candidate_keychain_keys_are_populated() {
        let accounts = ClientConfigParser::parse_str(SAMPLE_NEXTCLOUD_CFG).unwrap();
        let keys = accounts[0].candidate_keychain_keys();
        assert!(keys.contains(&"https://cloud.example.com:admin".to_string()));
    }
}
