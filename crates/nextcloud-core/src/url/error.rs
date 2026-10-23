use std::error::Error;
use std::fmt;

/// Reason a [`ServerUrl`](super::ServerUrl) or [`ApiPath`](super::ApiPath)
/// was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UrlError {
    /// The input carried no scheme.
    MissingScheme,
    /// The scheme is neither `http` nor `https`.
    UnsupportedScheme(String),
    /// The input carried no host.
    MissingHost,
    /// The base URL carried a query string or fragment.
    UnexpectedComponent(&'static str),
    /// A path component was not absolute.
    RelativePath(String),
    /// A path contained a traversal segment or an already-encoded delimiter.
    InvalidSegment(String),
}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingScheme => f.write_str("server URL has no scheme"),
            Self::UnsupportedScheme(scheme) => {
                write!(f, "server URL scheme `{scheme}` is not http or https")
            }
            Self::MissingHost => f.write_str("server URL has no host"),
            Self::UnexpectedComponent(component) => {
                write!(f, "server URL must not carry a {component}")
            }
            Self::RelativePath(path) => write!(f, "path `{path}` is not absolute"),
            Self::InvalidSegment(segment) => {
                write!(f, "path segment `{segment}` is not allowed")
            }
        }
    }
}

impl Error for UrlError {}

#[cfg(test)]
mod tests {
    use super::UrlError;

    #[test]
    fn display_names_the_offending_scheme() {
        let rendered = UrlError::UnsupportedScheme("ftp".to_owned()).to_string();
        assert!(rendered.contains("ftp"));
    }
}
