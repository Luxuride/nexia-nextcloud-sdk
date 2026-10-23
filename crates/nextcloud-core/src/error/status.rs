use std::error::Error;
use std::fmt;

use crate::http::StatusCode;
use crate::url::ApiPath;

/// A response carried a status the called endpoint cannot accept.
///
/// The body is kept because Nextcloud answers OCS failures with a JSON or
/// XML envelope that names the actual cause (`meta.statuscode: 997` for a
/// missing app, for example); dropping it would leave callers with only a
/// number.
#[derive(Debug)]
pub struct StatusError {
    method: &'static str,
    path: ApiPath,
    status: StatusCode,
    body: String,
}

impl StatusError {
    /// Records the failed call and keeps a bounded copy of the body.
    pub fn new(method: &'static str, path: ApiPath, status: StatusCode, body: &[u8]) -> Self {
        Self {
            method,
            path,
            status,
            body: truncate(body),
        }
    }

    /// Method that failed.
    pub const fn method(&self) -> &'static str {
        self.method
    }

    /// Path that failed.
    pub const fn path(&self) -> &ApiPath {
        &self.path
    }

    /// Status returned by the server.
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Body returned by the server, truncated to [`MAX_BODY_LEN`] bytes.
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// Longest body prefix retained in a [`StatusError`].
pub const MAX_BODY_LEN: usize = 512;

fn truncate(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    if text.chars().count() <= MAX_BODY_LEN {
        return text.into_owned();
    }
    let mut truncated: String = text.chars().take(MAX_BODY_LEN).collect();
    truncated.push('…');
    truncated
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} returned {}",
            self.method,
            self.path.as_str(),
            self.status
        )?;
        if !self.body.is_empty() {
            write!(f, ": {}", self.body)?;
        }
        Ok(())
    }
}

impl Error for StatusError {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::http::StatusCode;
    use crate::url::ApiPath;

    use super::{MAX_BODY_LEN, StatusError};

    fn error(status: u16, body: &[u8]) -> StatusError {
        StatusError::new(
            "GET",
            ApiPath::ocs("cloud/capabilities").unwrap(),
            StatusCode::new(status),
            body,
        )
    }

    #[test]
    fn display_names_method_path_and_status() {
        let rendered = error(404, b"not found").to_string();
        assert!(rendered.contains("GET"));
        assert!(rendered.contains("/ocs/v2.php/cloud/capabilities"));
        assert!(rendered.contains("404"));
    }

    #[test]
    fn long_bodies_are_truncated() {
        let body = vec![b'a'; MAX_BODY_LEN * 2];
        let error = error(500, &body);
        assert!(error.body().chars().count() <= MAX_BODY_LEN + 1);
    }

    #[test]
    fn is_a_std_error_without_a_source() {
        let error = error(500, b"");
        assert!(error.source().is_none());
    }
}
