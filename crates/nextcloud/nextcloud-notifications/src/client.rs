//! Asynchronous client for Nextcloud's Notifications API v2.

use nextcloud_ocs::{OcsEnvelope, OcsStatus};
use nextcloud_sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Headers, HttpMethod, Request, Response, StatusCode,
    StatusError, Transport,
};
use serde::Deserialize;

use crate::notification::Notification;

#[cfg(test)]
mod tests;

/// Relative path to Nextcloud's Notifications OCS API v2 beneath `/ocs/v2.php`.
///
/// See the official [Nextcloud Notifications API v2 documentation](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md).
pub const NOTIFICATIONS_API_PATH: &str = "apps/notifications/api/v2/notifications";

/// Header that marks a request as an OCS API call. Nextcloud rejects requests without it.
const OCS_REQUEST_HEADER: &str = "OCS-APIRequest";
/// Content negotiation header to request JSON responses.
const ACCEPT_HEADER: &str = "Accept";
const ACCEPT_JSON: &str = "application/json";

/// Client for interacting with Nextcloud's Notifications API (v2).
///
/// Executes calls through any [`Transport`] and applies credentials via an [`AuthProvider`].
/// Both are held by reference, allowing the client to be freely cloned or borrowed across
/// tasks without runtime dispatch or connection pooling overhead.
///
/// All endpoints interact with `/ocs/v2.php/apps/notifications/api/v2/notifications`.
///
/// See the [Nextcloud Notifications API v2 documentation](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md)
/// and [Nextcloud Developer Manual OCS Overview](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#notifications).
#[derive(Clone, Copy, Debug)]
pub struct NotificationClient<'a, T: Transport, A: AuthProvider> {
    transport: &'a T,
    auth: &'a A,
}

impl<'a, T: Transport, A: AuthProvider> NotificationClient<'a, T, A> {
    /// Binds a notification client to a transport and credential provider.
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

    /// Lists all notifications for the authenticated user.
    ///
    /// Sends `GET /ocs/v2.php/apps/notifications/api/v2/notifications`.
    ///
    /// Returns an empty list when no notifications exist, or when the server responds
    /// with `204 No Content` indicating polling should be reduced.
    ///
    /// See [Getting the notifications of a user](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#getting-the-notifications-of-a-user).
    pub async fn list(&self) -> Result<Vec<Notification>, Error> {
        tracing::debug!("Listing user notifications");
        let api_path = ApiPath::ocs(NOTIFICATIONS_API_PATH)?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        self.decode_list(api_path, response)
    }

    /// Retrieves a single notification by its `notification_id`.
    ///
    /// Sends `GET /ocs/v2.php/apps/notifications/api/v2/notifications/{id}`.
    ///
    /// See [Get a single notification for a user](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#get-a-single-notification-for-a-user).
    pub async fn get(&self, notification_id: u64) -> Result<Notification, Error> {
        tracing::debug!(notification_id, "Fetching notification details");
        let api_path = ApiPath::ocs(format!("{NOTIFICATIONS_API_PATH}/{notification_id}"))?;
        let headers = self.authenticated_headers();
        let request = Request::get(api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        self.decode_single(api_path, response)
    }

    /// Deletes/dismisses a single notification by its `notification_id`.
    ///
    /// Sends `DELETE /ocs/v2.php/apps/notifications/api/v2/notifications/{id}`.
    ///
    /// See [Deleting a notification for a user](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#deleting-a-notification-for-a-user).
    pub async fn delete(&self, notification_id: u64) -> Result<(), Error> {
        tracing::info!(notification_id, "Deleting notification");
        let api_path = ApiPath::ocs(format!("{NOTIFICATIONS_API_PATH}/{notification_id}"))?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        self.decode_empty(HttpMethod::Delete, api_path, response)
    }

    /// Deletes/dismisses all notifications for the authenticated user.
    ///
    /// Sends `DELETE /ocs/v2.php/apps/notifications/api/v2/notifications`.
    ///
    /// See [Deleting all notifications for a user](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#deleting-all-notifications-for-a-user).
    pub async fn delete_all(&self) -> Result<(), Error> {
        tracing::info!("Deleting all user notifications");
        let api_path = ApiPath::ocs(NOTIFICATIONS_API_PATH)?;
        let headers = self.authenticated_headers();
        let request = Request::new(HttpMethod::Delete, api_path.clone()).with_headers(headers);

        let response = self.transport.execute(request).await?;
        self.decode_empty(HttpMethod::Delete, api_path, response)
    }

    fn authenticated_headers(&self) -> Headers {
        let mut headers = Headers::new();
        headers.set(OCS_REQUEST_HEADER, "true");
        headers.set(ACCEPT_HEADER, ACCEPT_JSON);
        self.auth.authenticate(&mut headers);
        headers
    }

    fn decode_list(&self, path: ApiPath, response: Response) -> Result<Vec<Notification>, Error> {
        if response.status() == StatusCode::new(204) {
            return Ok(Vec::new());
        }

        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        if response.body().is_empty() {
            return Ok(Vec::new());
        }

        let envelope: OcsEnvelope<Option<Vec<Notification>>> =
            serde_json::from_slice(response.body())
                .map_err(|cause| {
                    DecodeError::new(
                        std::any::type_name::<OcsEnvelope<Vec<Notification>>>(),
                        cause,
                    )
                })
                .map_err(Error::Decode)?;

        if envelope.meta().verdict() == Some(OcsStatus::Failure) {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        Ok(envelope.into_data().unwrap_or_default())
    }

    fn decode_single(&self, path: ApiPath, response: Response) -> Result<Notification, Error> {
        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        let envelope: OcsEnvelope<NotificationOrList> = serde_json::from_slice(response.body())
            .map_err(|cause| {
                DecodeError::new(std::any::type_name::<OcsEnvelope<Notification>>(), cause)
            })
            .map_err(Error::Decode)?;

        if envelope.meta().verdict() == Some(OcsStatus::Failure) {
            return Err(Error::Status(StatusError::new(
                HttpMethod::Get.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        envelope.into_data().into_notification()
    }

    fn decode_empty(
        &self,
        method: HttpMethod,
        path: ApiPath,
        response: Response,
    ) -> Result<(), Error> {
        if !response.status().is_success() {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        if response.body().is_empty() {
            return Ok(());
        }

        let envelope: OcsEnvelope<serde_json::Value> = serde_json::from_slice(response.body())
            .map_err(|cause| {
                DecodeError::new(
                    std::any::type_name::<OcsEnvelope<serde_json::Value>>(),
                    cause,
                )
            })
            .map_err(Error::Decode)?;

        if envelope.meta().verdict() == Some(OcsStatus::Failure) {
            return Err(Error::Status(StatusError::new(
                method.as_str(),
                path,
                response.status(),
                response.body(),
            )));
        }

        Ok(())
    }
}

/// Helper to decode responses where `data` may be either a single notification
/// object or a single-item array.
#[derive(Deserialize)]
#[serde(untagged)]
enum NotificationOrList {
    Single(Notification),
    List(Vec<Notification>),
}

impl NotificationOrList {
    fn into_notification(self) -> Result<Notification, Error> {
        match self {
            Self::Single(notification) => Ok(notification),
            Self::List(mut list) => list.pop().ok_or_else(|| {
                Error::Decode(DecodeError::new(
                    std::any::type_name::<Notification>(),
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "notification response list was empty",
                    ),
                ))
            }),
        }
    }
}
