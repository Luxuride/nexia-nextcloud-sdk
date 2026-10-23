use std::fmt;

use super::UrlError;

/// Prefix of Nextcloud's OCS API.
pub const OCS_ROOT: &str = "/ocs/v2.php";
/// Prefix of Nextcloud's WebDAV tree.
pub const DAV_ROOT: &str = "/remote.php/dav";
/// Prefix of Nextcloud's public (no-login) endpoints.
pub const PUBLIC_ROOT: &str = "/index.php";

/// A request path beneath a [`ServerUrl`](super::ServerUrl).
///
/// Each variant owns the root its API area lives under, so callers describe
/// `cloud/capabilities` or `files/user` and never assemble `/ocs/v2.php` or
/// `/remote.php/dav` by hand.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ApiPath {
    /// `/ocs/v2.php/...`
    Ocs(String),
    /// `/remote.php/dav/...`
    Dav(String),
    /// `/index.php/...`
    Public(String),
}

impl ApiPath {
    /// Path beneath `/ocs/v2.php`.
    pub fn ocs(relative: impl AsRef<str>) -> Result<Self, UrlError> {
        Ok(Self::Ocs(OCS_ROOT.to_owned() + &normalise(relative.as_ref())?))
    }

    /// Path beneath `/remote.php/dav`.
    pub fn dav(relative: impl AsRef<str>) -> Result<Self, UrlError> {
        Ok(Self::Dav(DAV_ROOT.to_owned() + &normalise(relative.as_ref())?))
    }

    /// Path beneath `/index.php`.
    pub fn public(relative: impl AsRef<str>) -> Result<Self, UrlError> {
        Ok(Self::Public(
            PUBLIC_ROOT.to_owned() + &normalise(relative.as_ref())?,
        ))
    }

    /// Full path including its API root.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ocs(path) | Self::Dav(path) | Self::Public(path) => path,
        }
    }
}

impl fmt::Display for ApiPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn normalise(relative: &str) -> Result<String, UrlError> {
    let relative = relative.trim();
    for segment in relative.split('/') {
        match segment {
            "." | ".." => return Err(UrlError::InvalidSegment(segment.to_owned())),
            _ => {}
        }
    }
    let trimmed = relative.trim_start_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("/{trimmed}"))
}

#[cfg(test)]
mod tests {
    use super::ApiPath;

    #[test]
    fn ocs_path_is_prefixed_with_ocs_root() {
        let path = ApiPath::ocs("cloud/capabilities").unwrap();
        assert_eq!(path.as_str(), "/ocs/v2.php/cloud/capabilities");
    }

    #[test]
    fn leading_slash_is_tolerated() {
        let path = ApiPath::dav("/files/user").unwrap();
        assert_eq!(path.as_str(), "/remote.php/dav/files/user");
    }

    #[test]
    fn rejects_traversal_segments() {
        assert!(ApiPath::dav("files/../../etc").is_err());
    }

    #[test]
    fn empty_relative_path_is_the_root() {
        let path = ApiPath::ocs("   ").unwrap();
        assert_eq!(path.as_str(), "/ocs/v2.php");
    }
}
