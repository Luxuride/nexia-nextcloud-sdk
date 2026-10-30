//! Notification model representing a Nextcloud notification item.

use serde::{Deserialize, Serialize};

use crate::action::NotificationAction;
use crate::wire::{lossy_string, lossy_u64};

/// A Nextcloud notification item.
///
/// Returned by the Nextcloud Notifications OCS API v2.
///
/// See the official [Nextcloud Notifications API v2 Notification Element documentation](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#notification-element).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Notification {
    /// Unique identifier of the notification, used to retrieve or dismiss it.
    #[serde(deserialize_with = "lossy_u64")]
    pub notification_id: u64,

    /// Name of the Nextcloud app that triggered the notification (e.g. `"files_sharing"`).
    pub app: String,

    /// User ID of the user that receives the notification.
    pub user: String,

    /// ISO 8601 date and time when the notification was published.
    pub datetime: String,

    /// Type of the object the notification is about (e.g. `"remote_share"`).
    pub object_type: String,

    /// Identifier of the object within the originating app.
    #[serde(deserialize_with = "lossy_string")]
    pub object_id: String,

    /// Translated short subject presented to the user.
    pub subject: String,

    /// Optional translated detailed message text.
    #[serde(default)]
    pub message: String,

    /// Optional link to follow when the subject or message is clicked.
    #[serde(default)]
    pub link: String,

    /// Optional list of interactive actions available for this notification.
    #[serde(default)]
    pub actions: Vec<NotificationAction>,
}

impl Notification {
    /// Creates a new notification instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        notification_id: u64,
        app: String,
        user: String,
        datetime: String,
        object_type: String,
        object_id: String,
        subject: String,
        message: String,
        link: String,
        actions: Vec<NotificationAction>,
    ) -> Self {
        Self {
            notification_id,
            app,
            user,
            datetime,
            object_type,
            object_id,
            subject,
            message,
            link,
            actions,
        }
    }

    /// Unique identifier of the notification.
    pub const fn notification_id(&self) -> u64 {
        self.notification_id
    }

    /// Name of the Nextcloud app that triggered the notification.
    pub fn app(&self) -> &str {
        &self.app
    }

    /// User ID of the recipient.
    pub fn user(&self) -> &str {
        &self.user
    }

    /// ISO 8601 publication timestamp.
    pub fn datetime(&self) -> &str {
        &self.datetime
    }

    /// Classification of the object this notification references.
    pub fn object_type(&self) -> &str {
        &self.object_type
    }

    /// Identifier of the object within the originating app.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Short human-readable subject text.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Detailed message text, or empty if none was provided.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// URL link associated with the notification, or empty if none was provided.
    pub fn link(&self) -> &str {
        &self.link
    }

    /// Interactive actions available for this notification.
    pub fn actions(&self) -> &[NotificationAction] {
        &self.actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notification_from_json() {
        let json = r#"{
            "notification_id": 61,
            "app": "files_sharing",
            "user": "admin",
            "datetime": "2026-10-24T12:00:00+00:00",
            "object_type": "remote_share",
            "object_id": "13",
            "subject": "Share invitation received",
            "message": "User shared a folder",
            "link": "https://example.com/share/13",
            "actions": [
                {
                    "label": "Accept",
                    "link": "https://example.com/share/accept",
                    "type": "POST",
                    "primary": true
                }
            ]
        }"#;

        let n: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(n.notification_id(), 61);
        assert_eq!(n.app(), "files_sharing");
        assert_eq!(n.user(), "admin");
        assert_eq!(n.datetime(), "2026-10-24T12:00:00+00:00");
        assert_eq!(n.object_type(), "remote_share");
        assert_eq!(n.object_id(), "13");
        assert_eq!(n.subject(), "Share invitation received");
        assert_eq!(n.message(), "User shared a folder");
        assert_eq!(n.link(), "https://example.com/share/13");
        assert_eq!(n.actions().len(), 1);
        assert_eq!(n.actions()[0].label(), "Accept");
    }

    #[test]
    fn parses_numeric_object_id_and_string_notification_id() {
        let json = r#"{
            "notification_id": "99",
            "app": "core",
            "user": "alice",
            "datetime": "2026-10-24T12:00:00+00:00",
            "object_type": "user",
            "object_id": 1234,
            "subject": "Password reset"
        }"#;

        let n: Notification = serde_json::from_str(json).unwrap();
        assert_eq!(n.notification_id(), 99);
        assert_eq!(n.object_id(), "1234");
        assert_eq!(n.message(), "");
        assert_eq!(n.link(), "");
        assert!(n.actions().is_empty());
    }
}
