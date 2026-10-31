use std::fmt;

use zeroize::{Zeroize, Zeroizing};

/// A secret string whose [`fmt::Debug`] and [`fmt::Display`] implementations
/// always redact the underlying content to prevent accidental leakage.
///
/// The buffer is held in a [`Zeroizing`] wrapper, so it is wiped through volatile writes plus a
/// compiler fence when the value is dropped. A hand-written zeroing loop before `Vec::clear` is
/// not enough: the writes are dead stores that an optimising compiler is free to delete.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString {
    bytes: Zeroizing<Vec<u8>>,
}

impl SecretString {
    /// Creates a new secret wrapping `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            bytes: Zeroizing::new(value.into().into_bytes()),
        }
    }

    /// Exposes a reference to the secret plaintext.
    pub fn expose_secret(&self) -> &str {
        std::str::from_utf8(&self.bytes).unwrap_or_default()
    }

    /// Consumes the wrapper and returns the inner string.
    ///
    /// The returned [`String`] is the caller's to wipe: ownership leaves this type, and a `String`
    /// that is dropped normally does not clear its buffer. This method avoids leaving a *second*
    /// copy behind, which a clone-based conversion did.
    pub fn into_inner(mut self) -> String {
        match String::from_utf8(std::mem::take(&mut *self.bytes)) {
            Ok(text) => text,
            Err(error) => {
                let bytes = Zeroizing::new(error.into_bytes());
                String::from_utf8_lossy(&bytes).into_owned()
            }
        }
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Zeroize for SecretString {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_are_redacted() {
        let secret = SecretString::new("super-secret-token-1234");
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(secret.expose_secret(), "super-secret-token-1234");
    }

    #[test]
    fn into_inner_returns_the_plaintext_without_a_second_copy() {
        let secret = SecretString::new("super-secret-token-1234");
        assert_eq!(secret.into_inner(), "super-secret-token-1234");
    }

    #[test]
    fn zeroize_clears_the_buffer() {
        let mut secret = SecretString::new("super-secret-token-1234");
        secret.zeroize();
        assert_eq!(secret.expose_secret(), "");
        assert_eq!(secret.expose_secret().len(), 0);
    }

    #[test]
    fn cloned_secrets_stay_equal_and_redacted() {
        let secret = SecretString::new("super-secret-token-1234");
        let copy = secret.clone();
        assert_eq!(secret, copy);
        assert_eq!(format!("{copy:?}"), "[REDACTED]");
    }
}
