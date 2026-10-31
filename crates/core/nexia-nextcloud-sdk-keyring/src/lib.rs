//! Multi-platform secure keyring abstraction for the Nextcloud Rust SDK.
//!
//! Provides a uniform credential store interface across desktop environments:
//! - **Linux & Flatpak**: Powered by `oo7`, supporting direct D-Bus `org.freedesktop.secrets`
//!   and sandboxed `org.freedesktop.portal.Secret` inside Flatpak containers.
//! - **macOS**: Native Apple Keychain Services via Security.framework.
//! - **Windows**: Native Windows Credential Manager.
//! - **Testing/Headless**: In-memory [`MemoryKeyring`] backend.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

/// Platform-specific backend implementations.
pub mod backends;
/// High-level entry handle scoped to a service and account.
pub mod entry;
/// Keyring error types and classifications.
pub mod error;
/// Ordered lookup across several service and account name conventions.
pub mod lookup;
/// In-memory keyring backend for tests and headless environments.
pub mod memory;
/// Automatic platform native keyring selection.
pub mod platform;
/// Secret string wrapper with redacted debug and display formatting.
pub mod secret;
/// Common behavior trait for all keyring backends.
pub mod traits;

pub use entry::KeyringEntry;
pub use error::KeyringError;
pub use lookup::find_first_secret;
pub use memory::MemoryKeyring;
pub use platform::PlatformKeyring;
pub use secret::SecretString;
pub use traits::KeyringBackend;
