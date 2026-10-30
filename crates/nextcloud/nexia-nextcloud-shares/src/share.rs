use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

use crate::permissions::SharePermissions;
use crate::share_type::ShareType;

/// A Nextcloud share entity returned by the Files Sharing API.
///
/// Corresponds to the share records returned by `/ocs/v2.php/apps/files_sharing/api/v1/shares`.
///
/// See the [Nextcloud OCS Share API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html)
/// and [ocs-share-api.rst](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    /// Share identifier assigned by Nextcloud.
    #[serde(deserialize_with = "deserialize_id")]
    id: String,
    /// Type of recipient this share targets.
    share_type: ShareType,
    /// Target recipient identifier (username, group name, email, or federated address).
    #[serde(default)]
    share_with: Option<String>,
    /// Display name of the share recipient.
    #[serde(default)]
    share_with_displayname: Option<String>,
    /// Target path of the shared file or folder relative to the user's files root.
    path: String,
    /// Permissions granted to the recipient.
    permissions: SharePermissions,
    /// Public share token, if this is a public link share.
    ///
    /// Never printed in [`fmt::Debug`]; redacted as `<redacted>`.
    #[serde(default)]
    token: Option<String>,
    /// Public URL for accessing the share, if applicable.
    ///
    /// Never printed in [`fmt::Debug`]; redacted as `<redacted>` to prevent leaking share tokens.
    #[serde(default)]
    url: Option<String>,
    /// Expiration date or timestamp of the share, if set.
    #[serde(default)]
    expiration: Option<String>,
    /// Username of the share owner.
    #[serde(default)]
    uid_owner: Option<String>,
    /// Display name of the share owner.
    #[serde(default)]
    displayname_owner: Option<String>,
    /// Username of the file owner (may differ if reshared).
    #[serde(default)]
    uid_file_owner: Option<String>,
    /// Display name of the file owner.
    #[serde(default)]
    displayname_file_owner: Option<String>,
    /// Type of the shared item (`"file"` or `"folder"`).
    #[serde(default)]
    item_type: Option<String>,
    /// MIME type of the shared item.
    #[serde(default)]
    mimetype: Option<String>,
    /// Share creation timestamp (seconds since Unix epoch).
    #[serde(default)]
    stime: Option<i64>,
    /// Parent share ID, if this share was reshared.
    #[serde(default)]
    parent: Option<String>,
    /// File source ID within the storage engine.
    #[serde(default)]
    file_source: Option<i64>,
    /// Target file path in the recipient's space.
    #[serde(default)]
    file_target: Option<String>,
}

impl Share {
    /// Creates a new `Share` instance with required fields.
    pub fn new(
        id: impl Into<String>,
        share_type: ShareType,
        path: impl Into<String>,
        permissions: SharePermissions,
    ) -> Self {
        Self {
            id: id.into(),
            share_type,
            share_with: None,
            share_with_displayname: None,
            path: path.into(),
            permissions,
            token: None,
            url: None,
            expiration: None,
            uid_owner: None,
            displayname_owner: None,
            uid_file_owner: None,
            displayname_file_owner: None,
            item_type: None,
            mimetype: None,
            stime: None,
            parent: None,
            file_source: None,
            file_target: None,
        }
    }

    /// Share identifier assigned by Nextcloud.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Type of recipient this share targets.
    pub fn share_type(&self) -> ShareType {
        self.share_type
    }

    /// Target recipient identifier (username, group name, email, or federated address).
    pub fn share_with(&self) -> Option<&str> {
        self.share_with.as_deref()
    }

    /// Display name of the share recipient.
    pub fn share_with_displayname(&self) -> Option<&str> {
        self.share_with_displayname.as_deref()
    }

    /// Target path of the shared file or folder.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Permissions granted to the recipient.
    pub fn permissions(&self) -> SharePermissions {
        self.permissions
    }

    /// Public share token, if this is a public link share.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Sets the public share token.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Public URL for accessing the share, if applicable.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Sets the public URL for the share.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Formats the canonical public URL for this share given a Nextcloud server base URL.
    ///
    /// If the share entity already contains a full `url`, it is returned.
    /// Otherwise, if a `token` is present, formats as `"{server_base_url}/s/{token}"`.
    pub fn public_url(&self, server_base_url: &str) -> Option<String> {
        if let Some(ref url) = self.url
            && !url.is_empty()
        {
            return Some(url.clone());
        }
        self.token
            .as_ref()
            .map(|token| format!("{}/s/{}", server_base_url.trim_end_matches('/'), token))
    }

    /// Expiration date or timestamp of the share, if set.
    pub fn expiration(&self) -> Option<&str> {
        self.expiration.as_deref()
    }

    /// Username of the share owner.
    pub fn uid_owner(&self) -> Option<&str> {
        self.uid_owner.as_deref()
    }

    /// Display name of the share owner.
    pub fn displayname_owner(&self) -> Option<&str> {
        self.displayname_owner.as_deref()
    }

    /// Username of the file owner (may differ if reshared).
    pub fn uid_file_owner(&self) -> Option<&str> {
        self.uid_file_owner.as_deref()
    }

    /// Display name of the file owner.
    pub fn displayname_file_owner(&self) -> Option<&str> {
        self.displayname_file_owner.as_deref()
    }

    /// Type of the shared item (`"file"` or `"folder"`).
    pub fn item_type(&self) -> Option<&str> {
        self.item_type.as_deref()
    }

    /// MIME type of the shared item.
    pub fn mimetype(&self) -> Option<&str> {
        self.mimetype.as_deref()
    }

    /// Share creation timestamp (seconds since Unix epoch).
    pub fn stime(&self) -> Option<i64> {
        self.stime
    }

    /// Parent share ID, if this share was reshared.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    /// File source ID within the storage engine.
    pub fn file_source(&self) -> Option<i64> {
        self.file_source
    }

    /// Target file path in the recipient's space.
    pub fn file_target(&self) -> Option<&str> {
        self.file_target.as_deref()
    }
}

impl fmt::Debug for Share {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_token = self.token.as_ref().map(|_| "<redacted>");
        let redacted_url = self.url.as_ref().map(|_| "<redacted>");
        f.debug_struct("Share")
            .field("id", &self.id)
            .field("share_type", &self.share_type)
            .field("share_with", &self.share_with)
            .field("share_with_displayname", &self.share_with_displayname)
            .field("path", &self.path)
            .field("permissions", &self.permissions)
            .field("token", &redacted_token)
            .field("url", &redacted_url)
            .field("expiration", &self.expiration)
            .field("uid_owner", &self.uid_owner)
            .field("displayname_owner", &self.displayname_owner)
            .field("uid_file_owner", &self.uid_file_owner)
            .field("displayname_file_owner", &self.displayname_file_owner)
            .field("item_type", &self.item_type)
            .field("mimetype", &self.mimetype)
            .field("stime", &self.stime)
            .field("parent", &self.parent)
            .field("file_source", &self.file_source)
            .field("file_target", &self.file_target)
            .finish()
    }
}

fn deserialize_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    struct IdVisitor;

    impl serde::de::Visitor<'_> for IdVisitor {
        type Value = String;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a share ID as string or integer")
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<String, E> {
            Ok(v.to_owned())
        }

        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<String, E> {
            Ok(v)
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<String, E> {
            Ok(v.to_string())
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<String, E> {
            Ok(v.to_string())
        }
    }

    deserializer.deserialize_any(IdVisitor)
}

#[cfg(test)]
mod tests {
    use super::{Share, SharePermissions, ShareType};

    #[test]
    fn token_and_url_are_redacted_in_debug() {
        let mut share = Share::new(
            "42",
            ShareType::PublicLink,
            "/Documents/Report.pdf",
            SharePermissions::READ,
        );
        share.token = Some("super-secret-token-12345".to_string());
        share.url = Some("https://nextcloud.local/s/super-secret-token-12345".to_string());

        let debug_str = format!("{share:?}");
        assert!(
            !debug_str.contains("super-secret-token-12345"),
            "token leaked in debug output"
        );
        assert!(debug_str.contains("<redacted>"));
    }

    #[test]
    fn parses_share_json_with_numeric_id() {
        let json = r#"{
            "id": 42,
            "share_type": 0,
            "share_with": "alice",
            "share_with_displayname": "Alice",
            "path": "/Photos/Vacation",
            "permissions": 31,
            "token": null,
            "url": null,
            "expiration": null,
            "uid_owner": "bob",
            "displayname_owner": "Bob",
            "uid_file_owner": "bob",
            "displayname_file_owner": "Bob",
            "item_type": "folder",
            "mimetype": "httpd/unix-directory",
            "stime": 1600000000,
            "parent": null
        }"#;

        let share: Share = serde_json::from_str(json).unwrap();
        assert_eq!(share.id(), "42");
        assert_eq!(share.share_type(), ShareType::User);
        assert_eq!(share.share_with(), Some("alice"));
        assert_eq!(share.path(), "/Photos/Vacation");
        assert_eq!(share.permissions(), SharePermissions::ALL);
        assert_eq!(share.token(), None);
        assert_eq!(share.stime(), Some(1600000000));
    }

    #[test]
    fn parses_share_json_with_string_id() {
        let json = r#"{
            "id": "105",
            "share_type": 3,
            "path": "/SharedDoc.pdf",
            "permissions": 1,
            "token": "abcdef123456",
            "url": "https://example.com/s/abcdef123456"
        }"#;

        let share: Share = serde_json::from_str(json).unwrap();
        assert_eq!(share.id(), "105");
        assert_eq!(share.share_type(), ShareType::PublicLink);
        assert_eq!(share.token(), Some("abcdef123456"));
        assert_eq!(
            share.public_url("https://example.com"),
            Some("https://example.com/s/abcdef123456".to_string())
        );

        let token_only_share = Share::new(
            "106",
            ShareType::PublicLink,
            "/Doc.txt",
            SharePermissions::READ,
        )
        .with_token("token789");
        assert_eq!(
            token_only_share.public_url("https://mycloud.org/"),
            Some("https://mycloud.org/s/token789".to_string())
        );
    }
}
