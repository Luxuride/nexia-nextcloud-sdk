use nextcloud_sdk_core::{AuthProvider, Headers};

/// Applies no credentials at all.
///
/// Used for public endpoints such as shared links and the
/// capabilities/login-flow endpoints, which must not carry credentials even
/// when the client holds them. Nextcloud's OCS API requires the
/// `OCS-APIRequest: true` marker header, so it is set here to keep every
/// unauthenticated OCS call valid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anonymous;

impl Anonymous {
    /// Builds the provider.
    pub const fn new() -> Self {
        Self
    }
}

impl AuthProvider for Anonymous {
    fn authenticate(&self, headers: &mut Headers) {
        tracing::trace!("Applying Anonymous authentication marker");
        headers.remove("Authorization");
        headers.set("OCS-APIRequest", "true");
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_sdk_core::{AuthProvider, Header, Headers};

    use super::Anonymous;

    #[test]
    fn authenticate_removes_the_authorization_header() {
        let mut headers = Headers::new().with(Header::new("authorization", "Basic c2VjcmV0"));
        Anonymous.authenticate(&mut headers);
        assert_eq!(headers.get("Authorization"), None);
    }

    #[test]
    fn authenticate_sets_the_ocs_marker_header() {
        let mut headers = Headers::new();
        Anonymous.authenticate(&mut headers);
        assert_eq!(headers.get("OCS-APIRequest"), Some("true"));
    }

    #[test]
    fn authenticate_drops_credentials_without_touching_other_headers() {
        let mut headers = Headers::new()
            .with(Header::new("Authorization", "Bearer token"))
            .with(Header::new("Accept", "application/json"));
        Anonymous.authenticate(&mut headers);
        assert_eq!(headers.get("Authorization"), None);
        assert_eq!(headers.get("Accept"), Some("application/json"));
        assert_eq!(headers.len(), 2);
    }
}
