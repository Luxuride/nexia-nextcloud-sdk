use crate::http::{Headers, HttpMethod};
use crate::url::ApiPath;

/// A request described in Nextcloud terms: an API path plus method, headers
/// and an optional body. Resolving it to an absolute URL is the transport's
/// job, so request construction never deals with hosts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    method: HttpMethod,
    path: ApiPath,
    headers: Headers,
    body: Option<Vec<u8>>,
}

impl Request {
    /// A bodyless request for `path`.
    pub fn new(method: HttpMethod, path: ApiPath) -> Self {
        Self {
            method,
            path,
            headers: Headers::new(),
            body: None,
        }
    }

    /// A `GET` request.
    pub fn get(path: ApiPath) -> Self {
        Self::new(HttpMethod::Get, path)
    }

    /// A `POST` request.
    pub fn post(path: ApiPath) -> Self {
        Self::new(HttpMethod::Post, path)
    }

    /// Method this request will use.
    pub const fn method(&self) -> HttpMethod {
        self.method
    }

    /// Path this request targets.
    pub const fn path(&self) -> &ApiPath {
        &self.path
    }

    /// Headers set on this request.
    pub const fn headers(&self) -> &Headers {
        &self.headers
    }

    /// Body, if this request carries one.
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// Adds or replaces a header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.set(name, value);
        self
    }

    /// Replaces the whole header set, e.g. to apply credentials.
    #[must_use]
    pub fn with_headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Attaches a request body.
    #[must_use]
    pub fn body_bytes(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::http::HttpMethod;
    use crate::url::ApiPath;

    use super::Request;

    #[test]
    fn header_replaces_previous_value() {
        let request = Request::get(ApiPath::ocs("cloud/capabilities").unwrap())
            .header("Accept", "application/json")
            .header("accept", "text/xml");
        assert_eq!(request.headers().get("Accept"), Some("text/xml"));
        assert_eq!(request.headers().len(), 1);
    }

    #[test]
    fn post_requests_use_the_post_method() {
        let request = Request::post(ApiPath::ocs("cloud/user").unwrap());
        assert_eq!(request.method(), HttpMethod::Post);
        assert!(request.body().is_none());
    }

    #[test]
    fn body_bytes_are_stored_verbatim() {
        let request = Request::new(HttpMethod::Put, ApiPath::dav("files/user/a.txt").unwrap())
            .body_bytes(b"hello".to_vec());
        assert_eq!(request.body(), Some(&b"hello"[..]));
    }
}
