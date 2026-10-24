use crate::error::KeyringError;
use crate::memory::MemoryKeyring;
use crate::secret::SecretString;
use crate::traits::KeyringBackend;

#[derive(Clone, Debug)]
enum PlatformBackend {
    #[cfg(target_os = "linux")]
    Linux(crate::backends::linux::LinuxKeyring),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Desktop(crate::backends::desktop::DesktopKeyring),
    Memory(MemoryKeyring),
}

/// The default native keyring implementation for the current platform.
///
/// Automatically uses:
/// - `LinuxKeyring` with Flatpak portal support on Linux.
/// - `DesktopKeyring` on macOS (Keychain) and Windows (Credential Manager).
/// - [`MemoryKeyring`] as an in-memory fallback or when explicitly constructed.
#[derive(Clone, Debug)]
pub struct PlatformKeyring {
    backend: PlatformBackend,
}

impl Default for PlatformKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformKeyring {
    /// Creates a platform keyring using the native operating system backend.
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                backend: PlatformBackend::Linux(crate::backends::linux::LinuxKeyring::new()),
            }
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            Self {
                backend: PlatformBackend::Desktop(crate::backends::desktop::DesktopKeyring::new()),
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Self {
                backend: PlatformBackend::Memory(MemoryKeyring::new()),
            }
        }
    }

    /// Creates an explicit in-memory keyring, suitable for test suites or headless environments.
    pub fn in_memory() -> Self {
        Self {
            backend: PlatformBackend::Memory(MemoryKeyring::new()),
        }
    }
}

impl KeyringBackend for PlatformKeyring {
    async fn get_secret(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<SecretString>, KeyringError> {
        match &self.backend {
            #[cfg(target_os = "linux")]
            PlatformBackend::Linux(backend) => backend.get_secret(service, account).await,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            PlatformBackend::Desktop(backend) => backend.get_secret(service, account).await,
            PlatformBackend::Memory(backend) => backend.get_secret(service, account).await,
        }
    }

    async fn set_secret(
        &self,
        service: &str,
        account: &str,
        secret: &str,
    ) -> Result<(), KeyringError> {
        match &self.backend {
            #[cfg(target_os = "linux")]
            PlatformBackend::Linux(backend) => backend.set_secret(service, account, secret).await,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            PlatformBackend::Desktop(backend) => backend.set_secret(service, account, secret).await,
            PlatformBackend::Memory(backend) => backend.set_secret(service, account, secret).await,
        }
    }

    async fn delete_secret(&self, service: &str, account: &str) -> Result<(), KeyringError> {
        match &self.backend {
            #[cfg(target_os = "linux")]
            PlatformBackend::Linux(backend) => backend.delete_secret(service, account).await,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            PlatformBackend::Desktop(backend) => backend.delete_secret(service, account).await,
            PlatformBackend::Memory(backend) => backend.delete_secret(service, account).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn platform_in_memory_works() {
        let keyring = PlatformKeyring::in_memory();
        assert_eq!(keyring.get_secret("test", "user").await.unwrap(), None);

        keyring
            .set_secret("test", "user", "secret_pass")
            .await
            .unwrap();
        let val = keyring.get_secret("test", "user").await.unwrap().unwrap();
        assert_eq!(val.expose_secret(), "secret_pass");

        keyring.delete_secret("test", "user").await.unwrap();
        assert_eq!(keyring.get_secret("test", "user").await.unwrap(), None);
    }
}
