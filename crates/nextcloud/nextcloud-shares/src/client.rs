use nexia_nextcloud_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, Response, StatusError,
    Transport,
};
use nextcloud_ocs::{OcsEnvelope, OcsFormat, OcsStatus, Query};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::request::CreateShare;
use crate::share::Share;
use crate::share_type::ShareType;

/// Relative path to Nextcloud's Files Sharing OCS API beneath `/ocs/v2.php`.
///
/// See the [Nextcloud OCS Share API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html).
const SHARES_API_PATH: &str = "apps/files_sharing/api/v1/shares";

/// Header that marks a request as an OCS API call. Nextcloud rejects requests without it.
const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
/// Content negotiation header to request JSON responses.
const ACCEPT_HEADER: &str = "Accept";
const ACCEPT_JSON: &str = "application/json";
/// Form urlencoded content type header for POST requests.
const CONTENT_TYPE_HEADER: &str = "Content-Type";
const FORM_URLENCODED: &str = "application/x-www-form-urlencoded";

/// Client for interacting with Nextcloud's Files Sharing API.
///
/// Executes calls through any [`Transport`] and applies credentials via an [`AuthProvider`].
/// Both are held by reference, allowing the client to be freely cloned or borrowed across
/// tasks without runtime dispatch or connection pooling overhead.
///
/// Endpoints interact with `/ocs/v2.php/apps/files_sharing/api/v1/shares`.
///
/// See [Nextcloud OCS Share API](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html)
/// and [developer_manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst).
#[derive(Clone, Copy, Debug)]
pub struct ShareClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> ShareClient<'a, T, A> {
    /// Binds a share client to a transport and credential provider.
    pub const fn new(transport: &'a T, auth: &'a A) -> Self {
        Self { transport, auth }
    }

    /// The transport this client sends requests through.
    pub const fn transport(&self) -> &'a T {
        self.transport
    }

    /// The credential provider applied to every call.
    pub const fn auth(&self) -> &'a A {
        self.auth
    }

    /// Lists shares, optionally filtered by a specific file or folder `path`.
    ///
    /// Sends `GET /ocs/v2.php/apps/files_sharing/api/v1/shares`.
    ///
    /// When `path` is `Some(...)`, only shares for that exact path are returned.
    ///
    /// See [Get all Shares](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html#get-all-shares).
    pub async fn list(&self, path: Option<&str>) -> Result<Vec<Share>, Error> {
        tracing::debug!(?path, "Listing Nextcloud shares");
        let mut query = Query::new().format(OcsFormat::Json);
        if let Some(target_path) = path {
            query = query.param("path", target_path);
        }

        let api_path = ApiPath::ocs(format!("{SHARES_API_PATH}{}", query.build()))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        self.decode_envelope_data::<Vec<Share>>(HttpMethod::Get, api_path, response)
    }

    /// Retrieves information about a single share by its `share_id`.
    ///
    /// Sends `GET /ocs/v2.php/apps/files_sharing/api/v1/shares/{share_id}`.
    ///
    /// See [Get information about a known Share](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html#get-information-about-a-known-share).
    pub async fn get(&self, share_id: &str) -> Result<Share, Error> {
        tracing::debug!(share_id, "Fetching Nextcloud share details");
        let query = Query::new().format(OcsFormat::Json);
        let api_path = ApiPath::ocs(format!("{SHARES_API_PATH}/{share_id}{}", query.build()))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        let data = self.decode_envelope_data::<ShareOrList>(HttpMethod::Get, api_path, response)?;
        data.into_share().map_err(Error::Decode)
    }

    /// Deletes a share by its `share_id`.
    ///
    /// Sends `DELETE /ocs/v2.php/apps/files_sharing/api/v1/shares/{share_id}`.
    ///
    /// See [Delete Share](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html#delete-share).
    pub async fn delete(&self, share_id: &str) -> Result<(), Error> {
        tracing::info!(share_id, "Deleting Nextcloud share");
        let query = Query::new().format(OcsFormat::Json);
        let api_path = ApiPath::ocs(format!("{SHARES_API_PATH}/{share_id}{}", query.build()))?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        let _value: serde_json::Value =
            self.decode_envelope_data(HttpMethod::Delete, api_path, response)?;
        Ok(())
    }

    /// Creates a new share according to the given [`CreateShare`] parameters.
    ///
    /// Sends `POST /ocs/v2.php/apps/files_sharing/api/v1/shares`.
    ///
    /// See [Create a new Share](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-share-api.html#create-a-new-share).
    pub async fn create(&self, request: &CreateShare) -> Result<Share, Error> {
        tracing::info!(path = %request.path(), share_type = ?request.share_type(), "Creating Nextcloud share");
        let query = Query::new().format(OcsFormat::Json);
        let api_path = ApiPath::ocs(format!("{SHARES_API_PATH}{}", query.build()))?;
        let mut headers = self.authenticated_headers();
        headers.set(CONTENT_TYPE_HEADER, FORM_URLENCODED);

        let body = request.to_form_urlencoded();
        let post_req = Request::post(api_path.clone())
            .with_headers(headers)
            .body_bytes(body.into_bytes());

        let response = self.transport.execute(post_req).await?;
        let data =
            self.decode_envelope_data::<ShareOrList>(HttpMethod::Post, api_path, response)?;
        data.into_share().map_err(Error::Decode)
    }

    /// Convenience helper to create a public link share for `path`.
    pub async fn create_public_link(&self, path: &str) -> Result<Share, Error> {
        let request = CreateShare::public_link(path);
        self.create(&request).await
    }

    /// Retrieves an existing public link share for `path`, if one exists.
    pub async fn get_public_link(&self, path: &str) -> Result<Option<Share>, Error> {
        let shares = self.list(Some(path)).await?;
        Ok(shares
            .into_iter()
            .find(|s| s.share_type() == ShareType::PublicLink))
    }

    /// Retrieves an existing public link share for `path`, or creates a new one if none exists.
    pub async fn get_or_create_public_link(&self, path: &str) -> Result<Share, Error> {
        if let Some(existing) = self.get_public_link(path).await? {
            return Ok(existing);
        }
        self.create_public_link(path).await
    }

    /// Deletes any public link share currently associated with `path`.
    ///
    /// Returns `true` if a public link share was found and deleted, or `false` if none existed.
    pub async fn delete_public_link(&self, path: &str) -> Result<bool, Error> {
        if let Some(existing) = self.get_public_link(path).await? {
            self.delete(existing.id()).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn authenticated_headers(&self) -> Headers {
        let mut headers = Headers::new();
        headers.set(OCS_REQUEST_HEADER, "true");
        headers.set(ACCEPT_HEADER, ACCEPT_JSON);
        self.auth.authenticate(&mut headers);
        headers
    }

    fn decode_envelope_data<R: DeserializeOwned>(
        &self,
        method: HttpMethod,
        path: ApiPath,
        response: Response,
    ) -> Result<R, Error> {
        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        let envelope: OcsEnvelope<R> = serde_json::from_slice(response.body())
            .map_err(|cause| DecodeError::new(std::any::type_name::<OcsEnvelope<R>>(), cause))
            .map_err(Error::Decode)?;

        if envelope.meta().verdict() == Some(OcsStatus::Failure) {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        Ok(envelope.into_data())
    }
}

/// Helper to decode responses that Nextcloud may return as either a single
/// share object or a single-item array in `data`.
#[derive(Deserialize)]
#[serde(untagged)]
enum ShareOrList {
    Single(Box<Share>),
    List(Vec<Share>),
}

impl ShareOrList {
    fn into_share(self) -> Result<Share, DecodeError> {
        match self {
            Self::Single(share) => Ok(*share),
            Self::List(mut list) => list.pop().ok_or_else(|| {
                DecodeError::new(
                    std::any::type_name::<Share>(),
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "share response list was empty",
                    ),
                )
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use nexia_nextcloud_core::{
        AuthProvider, Error, Headers, HttpMethod, Request, Response, StatusCode, Transport,
    };

    use super::ShareClient;
    use crate::permissions::SharePermissions;
    use crate::request::CreateShare;
    use crate::share_type::ShareType;

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Clone, Debug)]
    struct FakeTransport {
        response: Response,
        seen: Arc<Mutex<Option<Request>>>,
    }

    impl FakeTransport {
        fn new(status: u16, body: &str) -> Self {
            Self {
                response: Response::new(StatusCode::new(status), Headers::new(), body.as_bytes()),
                seen: Arc::new(Mutex::new(None)),
            }
        }

        fn request(&self) -> Option<Request> {
            self.seen.lock().ok().and_then(|seen| seen.clone())
        }
    }

    impl Transport for FakeTransport {
        async fn execute(&self, request: Request) -> Result<Response, Error> {
            if let Ok(mut seen) = self.seen.lock() {
                *seen = Some(request);
            }
            Ok(self.response.clone())
        }
    }

    #[derive(Clone, Debug)]
    struct FakeAuth;

    impl AuthProvider for FakeAuth {
        fn authenticate(&self, headers: &mut Headers) {
            headers.set("Authorization", "Bearer mock-auth-token");
        }
    }

    const LIST_SHARES_JSON: &str = r#"{
        "ocs": {
            "meta": {
                "status": "ok",
                "statuscode": 200,
                "message": "OK",
                "totalitems": "2",
                "itemsperpage": ""
            },
            "data": [
                {
                    "id": "1",
                    "share_type": 0,
                    "share_with": "alice",
                    "path": "/Documents/Notes.txt",
                    "permissions": 31,
                    "token": null,
                    "url": null
                },
                {
                    "id": "2",
                    "share_type": 3,
                    "share_with": null,
                    "path": "/Photos/Vacation",
                    "permissions": 1,
                    "token": "tok12345",
                    "url": "https://nextcloud.test/s/tok12345"
                }
            ]
        }
    }"#;

    const SINGLE_SHARE_JSON: &str = r#"{
        "ocs": {
            "meta": {
                "status": "ok",
                "statuscode": 200,
                "message": "OK"
            },
            "data": {
                "id": "42",
                "share_type": 0,
                "share_with": "bob",
                "path": "/Reports/Q3.pdf",
                "permissions": 17,
                "uid_owner": "alice"
            }
        }
    }"#;

    const DELETE_OK_JSON: &str = r#"{
        "ocs": {
            "meta": {
                "status": "ok",
                "statuscode": 200,
                "message": "OK"
            },
            "data": []
        }
    }"#;

    const OCS_FAILURE_JSON: &str = r#"{
        "ocs": {
            "meta": {
                "status": "failure",
                "statuscode": 404,
                "message": "Wrong path, file/folder doesn't exist"
            },
            "data": []
        }
    }"#;

    #[test]
    fn list_shares_sends_correct_headers_and_parses_json() {
        let transport = FakeTransport::new(200, LIST_SHARES_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let shares = block_on(client.list(None)).unwrap();
        assert_eq!(shares.len(), 2);
        assert_eq!(shares[0].id(), "1");
        assert_eq!(shares[0].share_type(), ShareType::User);
        assert_eq!(shares[0].share_with(), Some("alice"));
        assert_eq!(shares[0].permissions(), SharePermissions::ALL);

        assert_eq!(shares[1].id(), "2");
        assert_eq!(shares[1].share_type(), ShareType::PublicLink);
        assert_eq!(shares[1].token(), Some("tok12345"));

        let req = transport.request().unwrap();
        assert_eq!(req.method(), HttpMethod::Get);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/files_sharing/api/v1/shares?format=json"
        );
        assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
        assert_eq!(req.headers().get("Accept"), Some("application/json"));
        assert_eq!(
            req.headers().get("Authorization"),
            Some("Bearer mock-auth-token")
        );
    }

    #[test]
    fn list_shares_with_path_filter() {
        let transport = FakeTransport::new(200, LIST_SHARES_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let _ = block_on(client.list(Some("/Documents/Notes.txt"))).unwrap();

        let req = transport.request().unwrap();
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/files_sharing/api/v1/shares?format=json&path=%2FDocuments%2FNotes.txt"
        );
    }

    #[test]
    fn get_share_by_id_parses_single_object() {
        let transport = FakeTransport::new(200, SINGLE_SHARE_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let share = block_on(client.get("42")).unwrap();
        assert_eq!(share.id(), "42");
        assert_eq!(share.share_with(), Some("bob"));
        assert_eq!(share.permissions().bits(), 17);

        let req = transport.request().unwrap();
        assert_eq!(req.method(), HttpMethod::Get);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/files_sharing/api/v1/shares/42?format=json"
        );
    }

    #[test]
    fn get_share_parses_array_wrapped_data() {
        let array_wrapped = r#"{
            "ocs": {
                "meta": { "status": "ok", "statuscode": 200, "message": "OK" },
                "data": [{
                    "id": "99",
                    "share_type": 1,
                    "share_with": "engineering",
                    "path": "/Projects",
                    "permissions": 31
                }]
            }
        }"#;

        let transport = FakeTransport::new(200, array_wrapped);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let share = block_on(client.get("99")).unwrap();
        assert_eq!(share.id(), "99");
        assert_eq!(share.share_type(), ShareType::Group);
        assert_eq!(share.share_with(), Some("engineering"));
    }

    #[test]
    fn delete_share_sends_delete_method() {
        let transport = FakeTransport::new(200, DELETE_OK_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        block_on(client.delete("42")).unwrap();

        let req = transport.request().unwrap();
        assert_eq!(req.method(), HttpMethod::Delete);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/files_sharing/api/v1/shares/42?format=json"
        );
        assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
    }

    #[test]
    fn create_share_sends_form_urlencoded_post() {
        let transport = FakeTransport::new(200, SINGLE_SHARE_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let create_req = CreateShare::user("/Reports/Q3.pdf", "bob")
            .permissions(SharePermissions::READ | SharePermissions::SHARE);

        let share = block_on(client.create(&create_req)).unwrap();
        assert_eq!(share.id(), "42");

        let req = transport.request().unwrap();
        assert_eq!(req.method(), HttpMethod::Post);
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/files_sharing/api/v1/shares?format=json"
        );
        assert_eq!(
            req.headers().get("Content-Type"),
            Some("application/x-www-form-urlencoded")
        );
        let body = String::from_utf8(req.body().unwrap().to_vec()).unwrap();
        assert_eq!(
            body,
            "path=%2FReports%2FQ3.pdf&shareType=0&shareWith=bob&permissions=17"
        );
    }

    #[test]
    fn ocs_failure_status_returns_error() {
        let transport = FakeTransport::new(200, OCS_FAILURE_JSON);
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let result = block_on(client.get("999"));
        match result {
            Err(Error::Status(status)) => {
                assert_eq!(status.method(), "GET");
                assert!(status.body().contains("Wrong path"));
            }
            other => panic!("expected Error::Status, got {other:?}"),
        }
    }

    #[test]
    fn http_error_status_returns_error() {
        let transport = FakeTransport::new(500, "Internal Server Error");
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let result = block_on(client.list(None));
        assert!(matches!(result, Err(Error::Status(_))));
    }

    #[test]
    fn malformed_json_returns_decode_error() {
        let transport = FakeTransport::new(200, "{ invalid json }");
        let auth = FakeAuth;
        let client = ShareClient::new(&transport, &auth);

        let result = block_on(client.list(None));
        assert!(matches!(result, Err(Error::Decode(_))));
    }
}
