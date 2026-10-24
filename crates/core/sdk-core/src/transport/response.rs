use crate::http::{Headers, StatusCode};

/// A raw response as it left the transport, before any API decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    status: StatusCode,
    headers: Headers,
    body: Vec<u8>,
}

impl Response {
    /// Builds a response from its parts.
    pub fn new(status: StatusCode, headers: Headers, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers,
            body: body.into(),
        }
    }

    /// Status code.
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Response headers.
    pub const fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Raw body bytes.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Body interpreted as UTF-8, for XML and diagnostic error pages.
    pub fn body_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.body)
    }

    /// Consumes the response, yielding its body.
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use crate::http::{Headers, StatusCode};

    use super::Response;

    #[test]
    fn exposes_status_headers_and_body() {
        let response = Response::new(StatusCode::new(200), Headers::new(), b"{}".to_vec());
        assert!(response.status().is_success());
        assert_eq!(response.body(), b"{}");
        assert_eq!(response.body_as_str().unwrap(), "{}");
    }

    #[test]
    fn invalid_utf8_is_reported_not_panicked() {
        let response = Response::new(StatusCode::new(200), Headers::new(), vec![0xff, 0xfe]);
        assert!(response.body_as_str().is_err());
    }
}
