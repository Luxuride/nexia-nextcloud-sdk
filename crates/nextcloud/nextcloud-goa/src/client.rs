//! Client for GNOME Online Accounts D-Bus service.

use std::collections::HashMap;

use nextcloud_auth::{AppPassword, BasicAuth, SecretString};
use zbus::zvariant::OwnedValue;

use crate::account::GoaAccount;
use crate::error::GoaError;
use crate::proxy::{FilesProxy, ObjectManagerProxy, PasswordBasedProxy};
use crate::url::parse_webdav_url;

/// Client for interacting with GNOME Online Accounts via D-Bus.
#[derive(Clone, Debug)]
pub struct GnomeOnlineAccounts {
    conn: zbus::Connection,
}

impl GnomeOnlineAccounts {
    /// Connects to the user's session D-Bus.
    pub async fn connect() -> Result<Self, GoaError> {
        let conn = zbus::Connection::session().await?;
        Ok(Self::from_connection(conn))
    }

    /// Creates a client from an existing D-Bus connection.
    pub const fn from_connection(conn: zbus::Connection) -> Self {
        Self { conn }
    }

    /// Accesses the underlying D-Bus connection.
    pub const fn connection(&self) -> &zbus::Connection {
        &self.conn
    }

    /// Discovers Nextcloud and ownCloud accounts configured in GNOME Online Accounts.
    ///
    /// This method calls `GetManagedObjects` on `/org/gnome/OnlineAccounts`,
    /// filters for objects implementing `org.gnome.OnlineAccounts.Account` with
    /// `ProviderType == "owncloud"`, extracts the username from `Identity`,
    /// parses `Files.Uri` into a [`nextcloud_sdk_core::ServerUrl`], queries the password
    /// from `org.gnome.OnlineAccounts.PasswordBased`, and returns the assembled
    /// accounts.
    ///
    /// # Errors
    ///
    /// - [`GoaError::NoAccountsFound`] if no matching accounts are discovered.
    /// - [`GoaError::DBus`] if a D-Bus call fails.
    /// - [`GoaError::MissingProperty`] if a required property is missing.
    /// - [`GoaError::Url`] if a WebDAV URI is malformed.
    pub async fn list_accounts(&self) -> Result<Vec<GoaAccount>, GoaError> {
        let object_manager = ObjectManagerProxy::new(&self.conn).await?;
        let managed_objects = object_manager.get_managed_objects().await?;

        let mut accounts = Vec::new();

        for (path, interfaces) in &managed_objects {
            let Some(account_props) = interfaces.get("org.gnome.OnlineAccounts.Account") else {
                continue;
            };

            let provider_type = extract_str(account_props, "ProviderType")?;
            if provider_type != "owncloud" && provider_type != "nextcloud" {
                continue;
            }

            let id = extract_str(account_props, "Id")?;
            let identity = extract_str(account_props, "Identity")?;
            let presentation_identity =
                extract_str(account_props, "PresentationIdentity").unwrap_or(identity);

            let uri = if let Some(files_props) = interfaces.get("org.gnome.OnlineAccounts.Files") {
                extract_str(files_props, "Uri")?.to_owned()
            } else {
                let files_proxy = FilesProxy::builder(&self.conn)
                    .path(path.as_ref())?
                    .build()
                    .await?;
                files_proxy.uri().await?
            };

            let server_url = parse_webdav_url(&uri)?;

            let password_proxy = PasswordBasedProxy::builder(&self.conn)
                .path(path.as_ref())?
                .build()
                .await?;
            let password = password_proxy.get_password("password").await?;

            let credentials = AppPassword::new(identity, SecretString::new(password));
            let auth = BasicAuth::from_app_password(credentials);

            accounts.push(GoaAccount::new(
                id,
                identity,
                server_url,
                auth,
                presentation_identity,
            ));
        }

        if accounts.is_empty() {
            return Err(GoaError::NoAccountsFound);
        }

        Ok(accounts)
    }
}

pub(crate) fn extract_str<'a>(
    props: &'a HashMap<String, OwnedValue>,
    key: &'static str,
) -> Result<&'a str, GoaError> {
    let val = props.get(key).ok_or(GoaError::MissingProperty(key))?;
    <&str>::try_from(val).map_err(|_| GoaError::MissingProperty(key))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedValue, Value};

    use super::extract_str;
    use crate::error::GoaError;

    #[test]
    fn extracts_string_property_successfully() {
        let mut map = HashMap::new();
        let val: OwnedValue = Value::from("account_1234").try_into().unwrap();
        map.insert("Id".to_string(), val);

        let extracted = extract_str(&map, "Id").unwrap();
        assert_eq!(extracted, "account_1234");
    }

    #[test]
    fn missing_key_returns_error() {
        let map = HashMap::new();
        let err = extract_str(&map, "MissingKey").unwrap_err();
        assert!(matches!(err, GoaError::MissingProperty("MissingKey")));
    }

    #[test]
    fn wrong_type_returns_error() {
        let mut map = HashMap::new();
        let val: OwnedValue = Value::from(42i32).try_into().unwrap();
        map.insert("Id".to_string(), val);

        let err = extract_str(&map, "Id").unwrap_err();
        assert!(matches!(err, GoaError::MissingProperty("Id")));
    }
}
