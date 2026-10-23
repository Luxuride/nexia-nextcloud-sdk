use std::error::Error;
use std::fmt;

/// A response body could not be turned into its typed form.
#[derive(Debug)]
pub struct DecodeError {
    target: &'static str,
    source: Box<dyn Error + Send + Sync>,
}

impl DecodeError {
    /// Wraps `source`, naming the type that failed to decode.
    pub fn new(target: &'static str, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            target,
            source: Box::new(source),
        }
    }

    /// Name of the type that could not be decoded.
    pub const fn target(&self) -> &'static str {
        self.target
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to decode `{}`: {}", self.target, self.source)
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::str::FromStr;

    use super::DecodeError;

    #[test]
    fn names_the_target_and_preserves_the_cause() {
        let cause = u32::from_str("not-a-number").unwrap_err();
        let error = DecodeError::new("Capabilities", cause);
        assert_eq!(error.target(), "Capabilities");
        assert!(error.source().is_some());
    }

    #[test]
    fn display_mentions_target_and_cause() {
        let cause = u32::from_str("not-a-number").unwrap_err();
        let rendered = DecodeError::new("Capabilities", cause).to_string();
        assert!(rendered.contains("Capabilities"));
        assert!(rendered.contains("failed to decode"));
    }
}
