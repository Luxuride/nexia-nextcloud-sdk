use nextcloud_sdk_core::{Error, Headers};

use crate::error::HeaderCopyError;

/// Header names whose values are credentials and must never be rendered.
///
/// `nextcloud_sdk_core::Header` already redacts its own `Debug` output; this list
/// additionally lets the value reach `reqwest` marked sensitive, so transport
/// level logging cannot print it either.
const CREDENTIAL_HEADERS: [&str; 3] = ["authorization", "proxy-authorization", "cookie"];

/// Copies the SDK header set onto a `reqwest` header map.
///
/// A rejected name or value becomes a typed [`Error::Transport`] naming the
/// header; the value itself is never quoted in the error.
pub(crate) fn to_header_map(headers: &Headers) -> Result<reqwest::header::HeaderMap, Error> {
    let mut map = reqwest::header::HeaderMap::with_capacity(headers.len());
    for header in headers.iter() {
        let name = reqwest::header::HeaderName::from_bytes(header.name().as_bytes()).map_err(
            |source| {
                Error::transport(HeaderCopyError::InvalidName {
                    name: header.name().to_owned(),
                    source,
                })
            },
        )?;
        let mut value =
            reqwest::header::HeaderValue::from_str(header.value()).map_err(|source| {
                Error::transport(HeaderCopyError::InvalidValue {
                    name: header.name().to_owned(),
                    source,
                })
            })?;
        if is_credential(name.as_str()) {
            value.set_sensitive(true);
        }
        map.insert(name, value);
    }
    Ok(map)
}

fn is_credential(name: &str) -> bool {
    CREDENTIAL_HEADERS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use nextcloud_sdk_core::{Header, Headers};

    use super::{is_credential, to_header_map};

    #[test]
    fn every_header_is_copied() {
        let headers = Headers::new()
            .with(Header::new("OCS-APIRequest", "true"))
            .with(Header::new("Accept", "application/json"))
            .with(Header::new("Depth", "1"));
        let map = to_header_map(&headers).expect("valid headers");
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("ocs-apirequest").unwrap(), "true");
        assert_eq!(map.get("accept").unwrap(), "application/json");
        assert_eq!(map.get("depth").unwrap(), "1");
    }

    #[test]
    fn credential_values_are_marked_sensitive() {
        let headers = Headers::new().with(Header::new("Authorization", "Basic dXNlcjpwYXNz"));
        let map = to_header_map(&headers).expect("valid header");
        assert!(map.get("authorization").unwrap().is_sensitive());
    }

    #[test]
    fn ordinary_values_stay_visible() {
        let headers = Headers::new().with(Header::new("Accept", "application/json"));
        let map = to_header_map(&headers).expect("valid header");
        assert!(!map.get("accept").unwrap().is_sensitive());
    }

    #[test]
    fn invalid_value_names_the_header_only() {
        let headers = Headers::new().with(Header::new("X-Trace", "line\nbreak"));
        let error = to_header_map(&headers).expect_err("newline is not a header value");
        assert!(error.is_retryable());
        assert!(error.to_string().contains("request failed"));
        assert!(!error.to_string().contains("line"));
    }

    #[test]
    fn credential_names_are_recognised_case_insensitively() {
        assert!(is_credential("authorization"));
        assert!(is_credential("Cookie"));
        assert!(!is_credential("accept"));
    }
}
