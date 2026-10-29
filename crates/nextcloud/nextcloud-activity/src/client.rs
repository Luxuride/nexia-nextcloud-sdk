//! Nextcloud Activity API client.
//!
//! Official documentation:
//! - [Activity API](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html)
//! - [Developer manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst)
//! - [Activity endpoint specification](https://github.com/nextcloud/activity/blob/master/docs/endpoint-v2.md)

use nexia_nextcloud_core::{ApiPath, AuthProvider, Error, Transport};
use nextcloud_ocs::OcsClient;

use crate::activity::Activity;
use crate::query::ActivityFilter;

/// Relative path to Nextcloud's Activity API beneath `/ocs/v2.php`.
const ACTIVITY_API_PATH: &str = "apps/activity/api/v2/activity";

/// Client for interacting with Nextcloud's Activity API v2.
///
/// Wraps an [`OcsClient`] by reference and provides high-level typed calls for:
/// - Listing activities matching filter criteria (`/ocs/v2.php/apps/activity/api/v2/activity`)
/// - Querying activities for a specific object type and ID
///
/// See the [Nextcloud Activity API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#endpoints)
/// and [developer_manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst).
#[derive(Clone, Copy, Debug)]
pub struct ActivityClient<'a, T: Transport, A: AuthProvider> {
    ocs: &'a OcsClient<T, A>,
}

impl<'a, T: Transport, A: AuthProvider> ActivityClient<'a, T, A> {
    /// Binds an activity client to an underlying [`OcsClient`].
    pub const fn new(ocs: &'a OcsClient<T, A>) -> Self {
        Self { ocs }
    }

    /// Borrows the underlying [`OcsClient`].
    pub const fn ocs(&self) -> &'a OcsClient<T, A> {
        self.ocs
    }

    /// Lists activities matching the provided [`ActivityFilter`].
    ///
    /// Sends `GET /ocs/v2.php/apps/activity/api/v2/activity` with query parameters.
    ///
    /// See [Endpoints](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#endpoints)
    /// and [Parameters](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#parameters).
    pub async fn list(&self, filter: &ActivityFilter) -> Result<Vec<Activity>, Error> {
        tracing::debug!("Listing activity stream");
        let path = ApiPath::ocs(ACTIVITY_API_PATH)?;
        self.ocs.get(path, filter.to_query()).await
    }

    /// Lists activities related to a specific object type and object ID.
    ///
    /// Helper that constructs an [`ActivityFilter`] with `object_type` and `object_id`
    /// and invokes [`Self::list`].
    ///
    /// Sends `GET /ocs/v2.php/apps/activity/api/v2/activity?format=json&object_type=...&object_id=...`.
    ///
    /// See [Parameters](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#parameters).
    pub async fn list_for_object(
        &self,
        object_type: &str,
        object_id: u64,
    ) -> Result<Vec<Activity>, Error> {
        let filter = ActivityFilter::new().object(object_type, object_id);
        self.list(&filter).await
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use nexia_nextcloud_core::{
        AuthProvider, Error, Headers, Request, Response, StatusCode, Transport,
    };
    use nextcloud_ocs::OcsClient;

    use super::ActivityClient;
    use crate::query::ActivityFilter;

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
            headers.set("Authorization", "Bearer test-activity-token");
        }
    }

    const ACTIVITY_LIST_RESPONSE: &str = r#"{
        "ocs": {
            "meta": {
                "status": "ok",
                "statuscode": 200,
                "message": "OK"
            },
            "data": [
                {
                    "activity_id": 1,
                    "datetime": "2026-10-24T12:00:00+00:00",
                    "app": "files",
                    "type": "file_created",
                    "user": "alice",
                    "subject": "alice created document.odt",
                    "message": null,
                    "link": "https://example.com/apps/files/?dir=/test",
                    "object_type": "files",
                    "object_id": 10,
                    "object_name": "/test/document.odt"
                },
                {
                    "activity_id": 2,
                    "datetime": "2026-10-24T12:05:00+00:00",
                    "app": "comments",
                    "type": "comment_added",
                    "user": "bob",
                    "subject": "bob commented on document.odt",
                    "message": "Looks great!",
                    "link": null,
                    "object_type": "files",
                    "object_id": 10,
                    "object_name": "/test/document.odt"
                }
            ]
        }
    }"#;

    const EMPTY_LIST_RESPONSE: &str = r#"{
        "ocs": {
            "meta": {
                "status": "ok",
                "statuscode": 200,
                "message": "OK"
            },
            "data": []
        }
    }"#;

    const OCS_FAILURE_RESPONSE: &str = r#"{
        "ocs": {
            "meta": {
                "status": "failure",
                "statuscode": 997,
                "message": "App not installed"
            },
            "data": []
        }
    }"#;

    #[test]
    fn list_fetches_and_decodes_activities() {
        let transport = FakeTransport::new(200, ACTIVITY_LIST_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = ActivityClient::new(&ocs);

        let filter = ActivityFilter::new().since(0).limit(10).sort("desc");
        let activities = block_on(client.list(&filter)).expect("list call succeeds");

        assert_eq!(activities.len(), 2);
        assert_eq!(activities[0].activity_id, 1);
        assert_eq!(activities[0].app, "files");
        assert_eq!(activities[0].activity_type, "file_created");
        assert_eq!(activities[0].user, "alice");
        assert_eq!(activities[0].subject, "alice created document.odt");
        assert_eq!(activities[0].message, None);
        assert_eq!(activities[0].object_id, Some(10));

        assert_eq!(activities[1].activity_id, 2);
        assert_eq!(activities[1].app, "comments");
        assert_eq!(activities[1].activity_type, "comment_added");
        assert_eq!(activities[1].user, "bob");
        assert_eq!(activities[1].message, Some("Looks great!".to_string()));

        let req = transport.request().expect("request was recorded");
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/activity/api/v2/activity?format=json&since=0&limit=10&sort=desc"
        );
        assert_eq!(req.headers().get("OCS-APIRequest"), Some("true"));
        assert_eq!(req.headers().get("Accept"), Some("application/json"));
        assert_eq!(
            req.headers().get("Authorization"),
            Some("Bearer test-activity-token")
        );
    }

    #[test]
    fn list_for_object_encodes_object_parameters() {
        let transport = FakeTransport::new(200, ACTIVITY_LIST_RESPONSE);
        let ocs = OcsClient::new(transport.clone(), FakeAuth);
        let client = ActivityClient::new(&ocs);

        let activities =
            block_on(client.list_for_object("files", 10)).expect("list_for_object succeeds");
        assert_eq!(activities.len(), 2);

        let req = transport.request().expect("request was recorded");
        assert_eq!(
            req.path().as_str(),
            "/ocs/v2.php/apps/activity/api/v2/activity?format=json&object_type=files&object_id=10"
        );
    }

    #[test]
    fn empty_activity_list_handled_gracefully() {
        let transport = FakeTransport::new(200, EMPTY_LIST_RESPONSE);
        let ocs = OcsClient::new(transport, FakeAuth);
        let client = ActivityClient::new(&ocs);

        let activities =
            block_on(client.list(&ActivityFilter::new())).expect("empty list succeeds");
        assert!(activities.is_empty());
    }

    #[test]
    fn ocs_failure_returns_status_error() {
        let transport = FakeTransport::new(200, OCS_FAILURE_RESPONSE);
        let ocs = OcsClient::new(transport, FakeAuth);
        let client = ActivityClient::new(&ocs);

        let result = block_on(client.list(&ActivityFilter::new()));
        assert!(result.is_err());
        match result {
            Err(Error::Status(err)) => {
                assert_eq!(err.status(), StatusCode::new(200));
            }
            other => panic!("expected Error::Status, got {other:?}"),
        }
    }
}
