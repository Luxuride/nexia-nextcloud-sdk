use std::fmt;

/// HTTP response status code.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusCode(u16);

impl StatusCode {
    /// Wraps a raw status code.
    pub const fn new(code: u16) -> Self {
        Self(code)
    }

    /// The raw numeric status.
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// `2xx`
    pub const fn is_success(self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    /// `3xx`
    pub const fn is_redirection(self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    /// `4xx`
    pub const fn is_client_error(self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    /// `5xx`
    pub const fn is_server_error(self) -> bool {
        self.0 >= 500 && self.0 < 600
    }

    /// `207 Multi-Status`, the WebDAV bulk response status.
    pub const fn is_multistatus(self) -> bool {
        self.0 == 207
    }
}

impl fmt::Debug for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StatusCode({})", self.0)
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::StatusCode;

    #[test]
    fn classifies_status_ranges() {
        assert!(StatusCode::new(200).is_success());
        assert!(StatusCode::new(304).is_redirection());
        assert!(StatusCode::new(404).is_client_error());
        assert!(StatusCode::new(503).is_server_error());
        assert!(!StatusCode::new(404).is_success());
    }

    #[test]
    fn recognises_multistatus() {
        assert!(StatusCode::new(207).is_multistatus());
        assert!(!StatusCode::new(200).is_multistatus());
    }
}
