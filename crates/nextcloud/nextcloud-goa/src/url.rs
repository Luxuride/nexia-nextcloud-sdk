//! URL parsing helpers for GNOME Online Accounts WebDAV endpoints.

use sdk_core::{ServerUrl, UrlError};

use crate::GoaError;

/// Parses a GNOME Online Accounts WebDAV URI into a Nextcloud base [`ServerUrl`].
///
/// GOA typically configures Nextcloud/ownCloud WebDAV URIs in formats such as:
/// - `https://cloud.example.com/remote.php/webdav/`
/// - `https://cloud.example.com/remote.php/dav/files/username/`
/// - `davs://cloud.example.com/remote.php/webdav/`
/// - `https://example.com/nextcloud/remote.php/webdav/`
///
/// This function normalises desktop-specific schemes (`dav://` and `davs://`),
/// strips Nextcloud's `/remote.php` routing endpoints, and produces a validated
/// base [`ServerUrl`].
pub fn parse_webdav_url(uri: &str) -> Result<ServerUrl, GoaError> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Err(GoaError::Url(UrlError::MissingHost));
    }

    // Convert GNOME dav(s):// schemes to standard http(s)://
    let normalized = if let Some(rest) = uri.strip_prefix("davs://") {
        format!("https://{rest}")
    } else if let Some(rest) = uri.strip_prefix("dav://") {
        format!("http://{rest}")
    } else {
        uri.to_owned()
    };

    // Strip `/remote.php` and any subsequent endpoint paths
    let base = match normalized.find("/remote.php") {
        Some(idx) => &normalized[..idx],
        None => &normalized,
    };

    let trimmed = base.trim_end_matches('/');
    let server_url = ServerUrl::parse(trimmed)?;
    Ok(server_url)
}

#[cfg(test)]
mod tests {
    use super::parse_webdav_url;
    use crate::GoaError;

    #[test]
    fn parses_standard_webdav_endpoint() {
        let parsed = parse_webdav_url("https://cloud.example.com/remote.php/webdav/").unwrap();
        assert_eq!(parsed.as_str(), "https://cloud.example.com");
        assert_eq!(parsed.authority(), "cloud.example.com");
        assert_eq!(parsed.path_prefix(), "");
    }

    #[test]
    fn parses_dav_files_endpoint() {
        let parsed =
            parse_webdav_url("https://cloud.example.com/remote.php/dav/files/alice/").unwrap();
        assert_eq!(parsed.as_str(), "https://cloud.example.com");
    }

    #[test]
    fn parses_davs_scheme() {
        let parsed = parse_webdav_url("davs://cloud.example.com/remote.php/webdav/").unwrap();
        assert_eq!(parsed.as_str(), "https://cloud.example.com");
        assert!(parsed.is_https());
    }

    #[test]
    fn parses_dav_insecure_scheme() {
        let parsed = parse_webdav_url("dav://insecure.example.com/remote.php/webdav/").unwrap();
        assert_eq!(parsed.as_str(), "http://insecure.example.com");
        assert!(!parsed.is_https());
    }

    #[test]
    fn parses_custom_port_and_subdirectory() {
        let parsed = parse_webdav_url("http://localhost:8080/nextcloud/remote.php/webdav").unwrap();
        assert_eq!(parsed.as_str(), "http://localhost:8080/nextcloud");
        assert_eq!(parsed.authority(), "localhost:8080");
        assert_eq!(parsed.path_prefix(), "/nextcloud");
    }

    #[test]
    fn parses_root_urls_without_remote_php() {
        let parsed1 = parse_webdav_url("https://cloud.example.com/").unwrap();
        assert_eq!(parsed1.as_str(), "https://cloud.example.com");

        let parsed2 = parse_webdav_url("https://cloud.example.com").unwrap();
        assert_eq!(parsed2.as_str(), "https://cloud.example.com");
    }

    #[test]
    fn rejects_invalid_urls() {
        assert!(matches!(parse_webdav_url(""), Err(GoaError::Url(_))));
        assert!(matches!(
            parse_webdav_url("cloud.example.com"),
            Err(GoaError::Url(_))
        ));
        assert!(matches!(
            parse_webdav_url("ftp://cloud.example.com"),
            Err(GoaError::Url(_))
        ));
    }
}
