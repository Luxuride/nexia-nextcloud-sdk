use std::error::Error as StdError;
use std::fmt;

/// Why a request header could not be handed to `reqwest`.
///
/// Only the header *name* is carried into the message: a rejected value is
/// frequently a credential, so it must not reach a log line or a `Debug`
/// rendering of the error.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) enum HeaderCopyError {
    /// The name is not a valid HTTP header name.
    InvalidName {
        /// Rejected header name.
        name: String,
        /// Cause reported by the header parser.
        source: reqwest::header::InvalidHeaderName,
    },
    /// The value contains characters a header value cannot carry.
    InvalidValue {
        /// Name of the header holding the rejected value.
        name: String,
        /// Cause reported by the header parser.
        source: reqwest::header::InvalidHeaderValue,
    },
}

impl fmt::Display for HeaderCopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { name, .. } => write!(f, "header `{name}` has an invalid name"),
            Self::InvalidValue { name, .. } => write!(f, "header `{name}` has an invalid value"),
        }
    }
}

impl StdError for HeaderCopyError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::InvalidName { source, .. } => Some(source),
            Self::InvalidValue { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HeaderCopyError;

    #[test]
    fn header_error_names_the_header_but_never_the_value() {
        let error = HeaderCopyError::InvalidValue {
            name: "authorization".to_owned(),
            source: reqwest::header::HeaderValue::from_str("bad\nvalue")
                .expect_err("newline is not a valid header value"),
        };
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("authorization"));
        assert!(!rendered.contains("bad"));
    }
}
