use std::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::error::StorageError;

const KEY_LEN: usize = 32;

/// A 256-bit database key.
///
/// SQLCipher accepts key material as a raw hex blob (`PRAGMA key =
/// "x'...'"`), which skips the passphrase KDF — appropriate here because the
/// key is already full-entropy random bytes, not a user password.
#[derive(Clone, PartialEq, Eq)]
pub struct StoreKey([u8; KEY_LEN]);

impl StoreKey {
    /// Draws a fresh key from the OS random source.
    pub fn generate() -> Result<Self, StorageError> {
        let mut bytes = [0u8; KEY_LEN];
        getrandom::fill(&mut bytes).map_err(StorageError::key_material)?;
        Ok(Self(bytes))
    }

    /// Builds a key from raw bytes.
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, StorageError> {
        let bytes = bytes.as_ref();
        let array: [u8; KEY_LEN] = bytes.try_into().map_err(|_| StorageError::MalformedKey {
            entry: format!("expected {KEY_LEN} bytes, got {}", bytes.len()),
        })?;
        Ok(Self(array))
    }

    /// Builds a key from its lower-case hex encoding, as stored in a keychain.
    pub fn from_hex(hex: &str, entry: impl Into<String>) -> Result<Self, StorageError> {
        let entry = entry.into();
        if hex.len() != KEY_LEN * 2 {
            return Err(StorageError::MalformedKey { entry });
        }
        let mut bytes = [0u8; KEY_LEN];
        for (index, chunk) in hex.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            let pair = std::str::from_utf8(chunk).map_err(|_| StorageError::MalformedKey {
                entry: entry.clone(),
            })?;
            bytes[index] =
                u8::from_str_radix(pair, 16).map_err(|_| StorageError::MalformedKey {
                    entry: entry.clone(),
                })?;
        }
        Ok(Self(bytes))
    }

    /// Lower-case hex encoding, for keychain storage and the SQLCipher pragma.
    ///
    /// The result is wrapped in [`Zeroizing`] because its only purpose is to carry the key into a
    /// keychain or a pragma string, and a plain `String` would be freed unzeroized.
    pub fn to_hex(&self) -> Zeroizing<String> {
        let mut hex = Zeroizing::new(String::with_capacity(KEY_LEN * 2));
        for byte in self.0 {
            hex.push_str(&format!("{byte:02x}"));
        }
        hex
    }

    /// The SQLCipher `PRAGMA key` statement that installs this key.
    pub fn pragma(&self) -> Zeroizing<String> {
        let hex = self.to_hex();
        Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", hex.as_str()))
    }
}

impl Drop for StoreKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for StoreKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoreKey(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::StoreKey;

    #[test]
    fn generated_keys_are_32_bytes_and_unique() {
        let first = StoreKey::generate().unwrap();
        let second = StoreKey::generate().unwrap();
        assert_ne!(first, second);
        assert_eq!(first.to_hex().len(), 64);
    }

    #[test]
    fn hex_round_trips() {
        let key = StoreKey::generate().unwrap();
        let restored = StoreKey::from_hex(key.to_hex().as_str(), "test").unwrap();
        assert_eq!(key, restored);
    }

    #[test]
    fn hex_rejects_wrong_length_and_non_hex() {
        assert!(StoreKey::from_hex("abcd", "test").is_err());
        let too_long = "z".repeat(64);
        assert!(StoreKey::from_hex(&too_long, "test").is_err());
    }

    #[test]
    fn from_bytes_requires_the_exact_length() {
        assert!(StoreKey::from_bytes([0u8; 31]).is_err());
        assert!(StoreKey::from_bytes([7u8; 32]).is_ok());
    }

    #[test]
    fn pragma_carries_the_hex_form() {
        let key = StoreKey::from_bytes([0xab; 32]).unwrap();
        assert_eq!(
            key.pragma().as_str(),
            format!("PRAGMA key = \"x'{}'\";", "ab".repeat(32))
        );
    }

    #[test]
    fn debug_never_shows_key_material() {
        let key = StoreKey::from_bytes([0x5a; 32]).unwrap();
        let rendered = format!("{key:?}");
        assert!(rendered.contains("redacted"));
        assert!(!rendered.contains("5a"));
    }
}
