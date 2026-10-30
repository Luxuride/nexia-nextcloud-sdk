//! D-Bus proxy interfaces for GNOME Online Accounts.

use std::collections::HashMap;
use zbus::proxy;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// A mapping of property names to their variant values.
pub type PropertyMap = HashMap<String, OwnedValue>;

/// A mapping of interface names to their property dictionaries.
pub type InterfaceMap = HashMap<String, PropertyMap>;

/// A mapping of object paths to their interfaces and properties.
pub type ManagedObjectsMap = HashMap<OwnedObjectPath, InterfaceMap>;

/// Proxy interface for `org.freedesktop.DBus.ObjectManager`.
#[proxy(
    default_service = "org.gnome.OnlineAccounts",
    default_path = "/org/gnome/OnlineAccounts",
    interface = "org.freedesktop.DBus.ObjectManager"
)]
pub trait ObjectManager {
    /// Retrieves all managed objects under `/org/gnome/OnlineAccounts`.
    fn get_managed_objects(&self) -> zbus::Result<ManagedObjectsMap>;
}

/// Proxy interface for `org.gnome.OnlineAccounts.Account`.
#[proxy(
    default_service = "org.gnome.OnlineAccounts",
    interface = "org.gnome.OnlineAccounts.Account"
)]
pub trait Account {
    /// Account unique identifier.
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// Technical identity (user name or email).
    #[zbus(property)]
    fn identity(&self) -> zbus::Result<String>;

    /// Presentation identity for display.
    #[zbus(property)]
    fn presentation_identity(&self) -> zbus::Result<String>;

    /// Provider type string (e.g. "owncloud").
    #[zbus(property)]
    fn provider_type(&self) -> zbus::Result<String>;
}

/// Proxy interface for `org.gnome.OnlineAccounts.Files`.
#[proxy(
    default_service = "org.gnome.OnlineAccounts",
    interface = "org.gnome.OnlineAccounts.Files"
)]
pub trait Files {
    /// WebDAV endpoint URI.
    #[zbus(property)]
    fn uri(&self) -> zbus::Result<String>;
}

/// Proxy interface for `org.gnome.OnlineAccounts.PasswordBased`.
#[proxy(
    default_service = "org.gnome.OnlineAccounts",
    interface = "org.gnome.OnlineAccounts.PasswordBased"
)]
pub trait PasswordBased {
    /// Retrieves the stored password.
    fn get_password(&self, id: &str) -> zbus::Result<String>;
}
