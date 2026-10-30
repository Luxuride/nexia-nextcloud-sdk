//! Notification action model representing interactive buttons and links.

use serde::{Deserialize, Serialize};

/// An interactive action button or link attached to a Nextcloud notification.
///
/// Actions represent follow-up operations a user can trigger directly from
/// the notification (such as accepting or declining a shared file).
///
/// See the official [Nextcloud Notifications API v2 Action Element documentation](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#action-element).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct NotificationAction {
    /// Translated short label of the action or button presented to the user.
    pub label: String,

    /// Target URL link to execute or navigate to when the action is triggered.
    pub link: String,

    /// HTTP method used for the action request (`"GET"`, `"POST"`, `"DELETE"`, `"PUT"`),
    /// or `"WEB"` for redirecting the browser.
    #[serde(rename = "type")]
    pub method: String,

    /// Whether this action is marked as the primary (default) action.
    #[serde(default, rename = "primary")]
    pub is_primary: bool,
}

impl NotificationAction {
    /// Creates a new notification action.
    pub const fn new(label: String, link: String, method: String, is_primary: bool) -> Self {
        Self {
            label,
            link,
            method,
            is_primary,
        }
    }

    /// Translated label of the action.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Target link of the action.
    pub fn link(&self) -> &str {
        &self.link
    }

    /// HTTP method or action type (`GET`, `POST`, `DELETE`, `PUT`, or `WEB`).
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Whether this is the primary action.
    pub const fn is_primary(&self) -> bool {
        self.is_primary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_action_from_json() {
        let json = r#"{
            "label": "Accept",
            "link": "https://example.com/share/accept",
            "type": "POST",
            "primary": true
        }"#;

        let action: NotificationAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.label(), "Accept");
        assert_eq!(action.link(), "https://example.com/share/accept");
        assert_eq!(action.method(), "POST");
        assert!(action.is_primary());
    }

    #[test]
    fn defaults_is_primary_to_false() {
        let json = r#"{
            "label": "Decline",
            "link": "https://example.com/share/decline",
            "type": "DELETE"
        }"#;

        let action: NotificationAction = serde_json::from_str(json).unwrap();
        assert_eq!(action.label(), "Decline");
        assert!(!action.is_primary());
    }

    #[test]
    fn serializes_matching_wire_field_names() {
        let action = NotificationAction::new(
            "View".to_owned(),
            "https://example.com/view".to_owned(),
            "GET".to_owned(),
            false,
        );

        let value = serde_json::to_value(&action).unwrap();
        assert_eq!(value["type"], "GET");
        assert_eq!(value["primary"], false);
    }
}
