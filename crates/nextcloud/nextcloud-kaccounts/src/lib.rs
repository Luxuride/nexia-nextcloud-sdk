//! KDE Plasma Online Accounts (KAccounts / Accounts-SSO) integration.
//!
//! Provides discovery of Nextcloud accounts configured inside KDE Plasma's System Settings,
//! reading from Accounts-SSO and resolving credentials via the platform keyring.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Discovered KDE Plasma online account representation.
pub mod account;
/// Credentials lookup helpers via `sdk-keyring`.
#[cfg(feature = "keyring")]
pub mod credentials;
/// Read-only SQLite database reader for `accounts.db`.
pub mod database;
/// Error classifications for KAccounts querying.
pub mod error;
/// File path discovery for `accounts.db`.
pub mod paths;

pub use account::KAccount;
#[cfg(feature = "keyring")]
pub use credentials::find_kaccount_credentials;
pub use database::KAccountsDatabase;
pub use error::KAccountsError;
pub use paths::{discover_accounts_db_paths, find_active_accounts_db};
