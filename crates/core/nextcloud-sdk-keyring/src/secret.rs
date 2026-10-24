use std::fmt;

/// A secret string whose [`fmt::Debug`] and [`fmt::Display`] implementations
/// always redact the underlying content to prevent accidental leakage.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString {
    bytes: Vec<u8>,
}

impl SecretString {
    /// Creates a new secret wrapping `value`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            bytes: value.into().into_bytes(),
        }
    }

    /// Exposes a reference to the secret plaintext.
    pub fn expose_secret(&self) -> &str {
        std::str::from_utf8(&self.bytes).unwrap_or_default()
    }

    /// Consumes the wrapper and returns the inner string.
    pub fn into_inner(self) -> String {
        String::from_utf8(self.bytes.clone()).unwrap_or_default()
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

impl Drop for SecretString {
    fn drop(&mut self) {
        for byte in self.bytes.iter_mut() {
            *byte = 0;
        }
        self.bytes.clear();
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
}
