use std::fmt;

/// One HTTP header, owned so it can cross thread and task boundaries.
#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    name: String,
    value: String,
}

impl Header {
    /// Builds a header from its name and value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Header name as given at construction.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Header value.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Whether the value must never appear in logs or `Debug` output.
    fn is_sensitive(&self) -> bool {
        matches!(
            self.name.to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
        )
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = if self.is_sensitive() {
            "<redacted>"
        } else {
            self.value.as_str()
        };
        f.debug_struct("Header")
            .field("name", &self.name)
            .field("value", &value)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Header;

    #[test]
    fn debug_redacts_credentials() {
        let header = Header::new("Authorization", "Basic dXNlcjpwYXNz");
        let rendered = format!("{header:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("dXNlcjpwYXNz"));
    }

    #[test]
    fn debug_shows_ordinary_headers() {
        let header = Header::new("Content-Type", "application/json");
        assert!(format!("{header:?}").contains("application/json"));
    }
}
