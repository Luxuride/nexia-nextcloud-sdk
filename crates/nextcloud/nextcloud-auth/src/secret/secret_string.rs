use std::fmt;

/// A string whose contents never appear in logs or `Debug` output.
///
/// The type on purpose has no `Display` and no `Serialize` impl: the only way
/// to read the plaintext is [`SecretString::expose`], which keeps every such
/// use visible at the call site.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps `value` unchanged.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the plaintext.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(\"<redacted>\")")
    }
}

#[cfg(test)]
mod tests {
    use super::SecretString;

    #[test]
    fn debug_redacts_the_secret() {
        let secret = SecretString::new("s3cr3t-app-password");
        let rendered = format!("{secret:?}");
        assert_eq!(rendered, "SecretString(\"<redacted>\")");
        assert!(!rendered.contains("s3cr3t-app-password"));
    }

    #[test]
    fn expose_returns_the_wrapped_value() {
        assert_eq!(SecretString::new("value").expose(), "value");
    }

    #[test]
    fn equality_compares_the_plaintext() {
        assert_eq!(SecretString::new("a"), SecretString::new("a"));
        assert_ne!(SecretString::new("a"), SecretString::new("b"));
    }
}
