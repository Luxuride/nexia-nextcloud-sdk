//! Nextcloud Notifications API (v2) client and domain models.
//!
//! Nextcloud provides an OCS API for listing, retrieving, and dismissing
//! user notifications beneath `/ocs/v2.php/apps/notifications/api/v2/notifications`.
//!
//! - [`NotificationClient`] — asynchronous client for fetching and deleting notifications.
//! - [`Notification`] — a Nextcloud notification item.
//! - [`NotificationAction`] — an interactive action button or link attached to a notification.
//!
//! See the official [Nextcloud Notifications API v2 documentation](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md)
//! and [Nextcloud Developer Manual OCS Overview](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#notifications).
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod action;
mod client;
mod notification;
mod wire;

pub use action::NotificationAction;
pub use client::{NOTIFICATIONS_API_PATH, NotificationClient};
pub use notification::Notification;
