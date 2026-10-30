use std::fmt;

use nexia_nextcloud_core::url::percent_encode;

use crate::permissions::SharePermissions;
use crate::share_type::ShareType;

/// Request builder for creating a new share via [`ShareClient::create`](crate::ShareClient::create).
///
/// Parameters match the official Nextcloud OCS Share API `POST` endpoint
/// at `/ocs/v2.php/apps/files_sharing/api/v1/shares`.
///
/// See the [Nextcloud OCS Share API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html)
/// and [ocs-share-api.rst](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst).
#[derive(Clone, PartialEq, Eq)]
pub struct CreateShare {
    path: String,
    share_type: ShareType,
    share_with: Option<String>,
    permissions: Option<SharePermissions>,
    password: Option<String>,
    public_upload: Option<bool>,
    expire_date: Option<String>,
    note: Option<String>,
    label: Option<String>,
}

impl CreateShare {
    /// Begins building a share creation request for `path` with `share_type`.
    pub fn new(path: impl Into<String>, share_type: ShareType) -> Self {
        Self {
            path: path.into(),
            share_type,
            share_with: None,
            permissions: None,
            password: None,
            public_upload: None,
            expire_date: None,
            note: None,
            label: None,
        }
    }

    /// Convenience constructor for sharing a file or folder with an individual user.
    pub fn user(path: impl Into<String>, user: impl Into<String>) -> Self {
        Self::new(path, ShareType::User).share_with(user)
    }

    /// Convenience constructor for creating a public link share.
    pub fn public_link(path: impl Into<String>) -> Self {
        Self::new(path, ShareType::PublicLink)
    }

    /// Sets the recipient of the share (user, group, email, or federated address).
    #[must_use]
    pub fn share_with(mut self, share_with: impl Into<String>) -> Self {
        self.share_with = Some(share_with.into());
        self
    }

    /// Sets custom permissions for the share.
    #[must_use]
    pub fn permissions(mut self, permissions: SharePermissions) -> Self {
        self.permissions = Some(permissions);
        self
    }

    /// Sets a password protecting a public link share.
    ///
    /// The password is never printed in [`fmt::Debug`]; it is redacted as `<redacted>`.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Enables or disables public upload on a public link share folder.
    #[must_use]
    pub fn public_upload(mut self, public_upload: bool) -> Self {
        self.public_upload = Some(public_upload);
        self
    }

    /// Sets an expiration date formatted as `YYYY-MM-DD`.
    #[must_use]
    pub fn expire_date(mut self, expire_date: impl Into<String>) -> Self {
        self.expire_date = Some(expire_date.into());
        self
    }

    /// Adds a note for the share recipient.
    #[must_use]
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Adds a label for the share.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Path targeted by this share request.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Share type targeted by this share request.
    pub fn share_type(&self) -> ShareType {
        self.share_type
    }

    /// Recipient of this share request, if set.
    pub fn share_with_value(&self) -> Option<&str> {
        self.share_with.as_deref()
    }

    /// Permissions configured for this share, if explicitly set.
    pub fn permissions_value(&self) -> Option<SharePermissions> {
        self.permissions
    }

    /// Password configured for this share, if set.
    pub fn password_value(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Public upload setting configured for this share, if explicitly set.
    pub fn public_upload_value(&self) -> Option<bool> {
        self.public_upload
    }

    /// Expiration date configured for this share, if set.
    pub fn expire_date_value(&self) -> Option<&str> {
        self.expire_date.as_deref()
    }

    /// Note configured for this share, if set.
    pub fn note_value(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Label configured for this share, if set.
    pub fn label_value(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Serialises the request into `application/x-www-form-urlencoded` body bytes.
    pub fn to_form_urlencoded(&self) -> String {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        pairs.push(("path", self.path.clone()));
        pairs.push(("shareType", self.share_type.as_u8().to_string()));

        if let Some(with) = &self.share_with {
            pairs.push(("shareWith", with.clone()));
        }
        if let Some(perms) = self.permissions {
            pairs.push(("permissions", perms.bits().to_string()));
        }
        if let Some(pw) = &self.password {
            pairs.push(("password", pw.clone()));
        }
        if let Some(upload) = self.public_upload {
            pairs.push((
                "publicUpload",
                if upload {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
            ));
        }
        if let Some(expire) = &self.expire_date {
            pairs.push(("expireDate", expire.clone()));
        }
        if let Some(n) = &self.note {
            pairs.push(("note", n.clone()));
        }
        if let Some(l) = &self.label {
            pairs.push(("label", l.clone()));
        }

        encode_form_pairs(&pairs)
    }
}

impl fmt::Debug for CreateShare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_password = self.password.as_ref().map(|_| "<redacted>");
        f.debug_struct("CreateShare")
            .field("path", &self.path)
            .field("share_type", &self.share_type)
            .field("share_with", &self.share_with)
            .field("permissions", &self.permissions)
            .field("password", &redacted_password)
            .field("public_upload", &self.public_upload)
            .field("expire_date", &self.expire_date)
            .field("note", &self.note)
            .field("label", &self.label)
            .finish()
    }
}

fn encode_form_pairs(pairs: &[(&str, String)]) -> String {
    let mut out = String::new();
    for (k, v) in pairs {
        if !out.is_empty() {
            out.push('&');
        }
        out.push_str(&percent_encode(k));
        out.push('=');
        out.push_str(&percent_encode(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CreateShare, SharePermissions};

    #[test]
    fn password_is_redacted_in_debug() {
        let req = CreateShare::public_link("/SecretFolder").password("super_secret_password_987!");

        let dbg = format!("{req:?}");
        assert!(
            !dbg.contains("super_secret_password_987!"),
            "password leaked in debug output"
        );
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn form_urlencoding_basic() {
        let req = CreateShare::user("/Photos/Trip", "alice");
        let encoded = req.to_form_urlencoded();
        assert_eq!(encoded, "path=%2FPhotos%2FTrip&shareType=0&shareWith=alice");
    }

    #[test]
    fn form_urlencoding_full_public_link() {
        let req = CreateShare::public_link("/Uploads")
            .permissions(SharePermissions::READ | SharePermissions::CREATE)
            .password("pass123")
            .public_upload(true)
            .expire_date("2026-12-31")
            .note("Project files")
            .label("Upload portal");
        let encoded = req.to_form_urlencoded();
        assert_eq!(
            encoded,
            "path=%2FUploads&shareType=3&permissions=5&password=pass123&publicUpload=true&expireDate=2026-12-31&note=Project%20files&label=Upload%20portal"
        );
    }
}
