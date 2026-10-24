use nextcloud_sdk_core::{Error, HttpMethod};

/// Type `reqwest` reports when a method name is not a valid HTTP token.
type InvalidMethod = <reqwest::Method as std::str::FromStr>::Err;

/// Maps a Nextcloud method onto its `reqwest` (`http`) counterpart.
///
/// The WebDAV verbs have no constants on `reqwest::Method`, so they are parsed
/// from the same uppercase wire names the rest of the SDK uses. Every
/// [`HttpMethod`] variant yields a valid token, so the error case is
/// unreachable in practice — it exists so no call site needs to assume that
/// and the failure still surfaces as a typed [`Error`] instead of a panic.
pub(crate) fn to_reqwest(method: HttpMethod) -> Result<reqwest::Method, Error> {
    let mapped = match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
        HttpMethod::Propfind
        | HttpMethod::Proppatch
        | HttpMethod::Mkcol
        | HttpMethod::Move
        | HttpMethod::Copy
        | HttpMethod::Report => {
            return reqwest::Method::from_bytes(method.as_str().as_bytes())
                .map_err(|source| Error::transport(MethodRejected { method, source }));
        }
    };
    Ok(mapped)
}

/// Cause recorded when a wire name is rejected as an HTTP method token.
#[derive(Debug)]
struct MethodRejected {
    method: HttpMethod,
    source: InvalidMethod,
}

impl std::fmt::Display for MethodRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}` is not a valid HTTP method token",
            self.method.as_str()
        )
    }
}

impl std::error::Error for MethodRejected {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use nextcloud_sdk_core::HttpMethod;

    use super::to_reqwest;

    #[test]
    fn plain_verbs_map_to_the_reqwest_constants() {
        assert_eq!(to_reqwest(HttpMethod::Get).unwrap(), reqwest::Method::GET);
        assert_eq!(to_reqwest(HttpMethod::Post).unwrap(), reqwest::Method::POST);
        assert_eq!(to_reqwest(HttpMethod::Put).unwrap(), reqwest::Method::PUT);
        assert_eq!(
            to_reqwest(HttpMethod::Patch).unwrap(),
            reqwest::Method::PATCH
        );
        assert_eq!(
            to_reqwest(HttpMethod::Delete).unwrap(),
            reqwest::Method::DELETE
        );
    }

    #[test]
    fn webdav_verbs_keep_their_wire_name() {
        for method in [
            HttpMethod::Propfind,
            HttpMethod::Proppatch,
            HttpMethod::Mkcol,
            HttpMethod::Move,
            HttpMethod::Copy,
            HttpMethod::Report,
        ] {
            assert_eq!(to_reqwest(method).unwrap().as_str(), method.as_str());
        }
    }
}
