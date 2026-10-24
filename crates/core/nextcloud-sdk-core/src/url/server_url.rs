use std::fmt;

use super::{ApiPath, UrlError};

/// Base URL of a Nextcloud instance, e.g. `https://cloud.example.org`.
///
/// The value is normalised once at construction: the scheme is lower-cased,
/// an explicit default port is dropped, and trailing slashes are trimmed so
/// joining a path never produces a double slash.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ServerUrl {
    normalised: String,
    scheme: Scheme,
    authority: String,
    path_prefix: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Scheme {
    Http,
    Https,
}

impl Scheme {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }

    fn from_str(value: &str) -> Result<Self, UrlError> {
        match value.to_ascii_lowercase().as_str() {
            "http" => Ok(Self::Http),
            "https" => Ok(Self::Https),
            other => Err(UrlError::UnsupportedScheme(other.to_owned())),
        }
    }
}

impl ServerUrl {
    /// Parses and normalises a base URL.
    pub fn parse(input: impl AsRef<str>) -> Result<Self, UrlError> {
        let input = input.as_ref().trim();
        let (scheme, rest) = input.split_once("://").ok_or(UrlError::MissingScheme)?;
        let scheme = Scheme::from_str(scheme)?;

        if rest.contains('?') {
            return Err(UrlError::UnexpectedComponent("query string"));
        }
        if rest.contains('#') {
            return Err(UrlError::UnexpectedComponent("fragment"));
        }

        let (authority, path_prefix) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        if authority.is_empty() {
            return Err(UrlError::MissingHost);
        }

        let authority = normalise_authority(authority, scheme)?;
        let path_prefix = path_prefix.trim_end_matches('/').to_owned();
        validate_prefix(&path_prefix)?;

        let normalised = if path_prefix.is_empty() {
            format!("{}://{authority}", scheme.as_str())
        } else {
            format!("{}://{authority}{path_prefix}", scheme.as_str())
        };

        Ok(Self {
            normalised,
            scheme,
            authority,
            path_prefix,
        })
    }

    /// Normalised base URL without a trailing slash.
    pub fn as_str(&self) -> &str {
        &self.normalised
    }

    /// Scheme this URL uses.
    pub const fn is_https(&self) -> bool {
        matches!(self.scheme, Scheme::Https)
    }

    /// `host` or `host:port`, with the default port omitted.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Path the instance is mounted under, empty for a root install.
    pub fn path_prefix(&self) -> &str {
        &self.path_prefix
    }

    /// Absolute URL for `path`.
    pub fn join(&self, path: &ApiPath) -> String {
        format!("{}{}", self.normalised, path.as_str())
    }
}

impl fmt::Display for ServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalised)
    }
}

impl fmt::Debug for ServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServerUrl({})", self.normalised)
    }
}

fn normalise_authority(authority: &str, scheme: Scheme) -> Result<String, UrlError> {
    let authority = authority.to_ascii_lowercase();
    match authority.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            let port: u16 = port.parse().map_err(|_| UrlError::MissingHost)?;
            if port == scheme.default_port() {
                Ok(host.to_owned())
            } else {
                Ok(format!("{host}:{port}"))
            }
        }
        _ => Ok(authority),
    }
}

fn validate_prefix(prefix: &str) -> Result<(), UrlError> {
    for segment in prefix.split('/').filter(|segment| !segment.is_empty()) {
        if segment == "." || segment == ".." {
            return Err(UrlError::InvalidSegment(segment.to_owned()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ServerUrl;

    #[test]
    fn trims_trailing_slash() {
        let url = ServerUrl::parse("https://cloud.example.org/").unwrap();
        assert_eq!(url.as_str(), "https://cloud.example.org");
    }

    #[test]
    fn drops_default_https_port() {
        let url = ServerUrl::parse("https://cloud.example.org:443").unwrap();
        assert_eq!(url.authority(), "cloud.example.org");
    }

    #[test]
    fn keeps_non_default_port() {
        let url = ServerUrl::parse("http://localhost:8080").unwrap();
        assert_eq!(url.authority(), "localhost:8080");
        assert!(!url.is_https());
    }

    #[test]
    fn keeps_installation_prefix() {
        let url = ServerUrl::parse("https://example.org/nextcloud/").unwrap();
        assert_eq!(url.path_prefix(), "/nextcloud");
        assert_eq!(url.as_str(), "https://example.org/nextcloud");
    }

    #[test]
    fn rejects_missing_scheme() {
        assert!(ServerUrl::parse("cloud.example.org").is_err());
    }

    #[test]
    fn rejects_query_string() {
        assert!(ServerUrl::parse("https://example.org/?a=1").is_err());
    }

    #[test]
    fn rejects_traversal_in_prefix() {
        assert!(ServerUrl::parse("https://example.org/a/../b").is_err());
    }
}
