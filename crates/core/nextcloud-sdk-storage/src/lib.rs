//! Encrypted local storage for the SDK.
//!
//! Two databases are kept side by side — one for credentials, one for cached
//! server data — at paths the application chooses ([`StoragePaths`]). Both are
//! SQLCipher databases opened with a 256-bit [`StoreKey`]; the key itself is
//! never written to disk, it lives in the platform keychain via `nextcloud-sdk-keyring`
//! ([`KeychainKeyStore`]) or in whatever store the embedding application
//! supplies through [`KeyStore`].
//!
//! ```no_run
//! use nextcloud_sdk_storage::{KeychainKeyStore, KeyStore, StoragePaths};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let paths = StoragePaths::default_xdg()?;
//! let keys = KeychainKeyStore::new("dev.example.app", "nextcloud-rust-sdk");
//! let auth = paths.open_auth_db(&keys.load_or_create().await?)?;
//! # let _ = auth;
//! # Ok(())
//! # }
//! ```
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod database;
mod error;
mod key;
mod key_store;
mod paths;
mod schema;

#[cfg(feature = "keychain")]
mod keychain;

pub use database::EncryptedDatabase;
pub use error::StorageError;
pub use key::StoreKey;
pub use key_store::KeyStore;
pub use paths::StoragePaths;
pub use schema::{CacheSchema, ConfigSchema, CredentialSchema};

#[cfg(feature = "keychain")]
pub use keychain::KeychainKeyStore;
