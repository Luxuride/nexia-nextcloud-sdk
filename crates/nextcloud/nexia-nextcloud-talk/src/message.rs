//! Chat message model for Nextcloud Talk.

use serde::{Deserialize, Serialize};

/// A chat message within a Nextcloud Talk conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique identifier of the chat message.
    #[serde(deserialize_with = "crate::wire::lossy_u64")]
    pub id: u64,
    /// Token of the conversation this message belongs to.
    pub token: String,
    /// Actor type of the sender (e.g. `"users"`, `"bots"`, `"guests"`).
    #[serde(rename = "actorType", alias = "actor_type")]
    pub actor_type: String,
    /// Actor identifier of the sender (e.g. user ID).
    #[serde(rename = "actorId", alias = "actor_id")]
    pub actor_id: String,
    /// Display name of the sender.
    #[serde(rename = "actorDisplayName", alias = "actor_display_name")]
    pub actor_display_name: String,
    /// Unix timestamp when the message was posted (in seconds).
    #[serde(deserialize_with = "crate::wire::lossy_i64")]
    pub timestamp: i64,
    /// Text content of the message.
    pub message: String,
    /// System message identifier if this is a system event, otherwise empty.
    #[serde(rename = "systemMessage", alias = "system_message", default)]
    pub system_message: String,
}

impl ChatMessage {
    /// Creates a new [`ChatMessage`].
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        token: impl Into<String>,
        actor_type: impl Into<String>,
        actor_id: impl Into<String>,
        actor_display_name: impl Into<String>,
        timestamp: i64,
        message: impl Into<String>,
        system_message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            token: token.into(),
            actor_type: actor_type.into(),
            actor_id: actor_id.into(),
            actor_display_name: actor_display_name.into(),
            timestamp,
            message: message.into(),
            system_message: system_message.into(),
        }
    }

    /// Returns `true` if this message represents a system event.
    pub fn is_system_message(&self) -> bool {
        !self.system_message.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_deserialization_from_talk_v1_json() {
        let json = r#"{
            "id": 1024,
            "token": "room_abc123",
            "actorType": "users",
            "actorId": "alice",
            "actorDisplayName": "Alice Cooper",
            "timestamp": 1718000000,
            "message": "Hello everyone!",
            "systemMessage": ""
        }"#;

        let msg: ChatMessage = serde_json::from_str(json).expect("valid chat message json");
        assert_eq!(msg.id, 1024);
        assert_eq!(msg.token, "room_abc123");
        assert_eq!(msg.actor_type, "users");
        assert_eq!(msg.actor_id, "alice");
        assert_eq!(msg.actor_display_name, "Alice Cooper");
        assert_eq!(msg.timestamp, 1718000000);
        assert_eq!(msg.message, "Hello everyone!");
        assert_eq!(msg.system_message, "");
        assert!(!msg.is_system_message());
    }

    #[test]
    fn chat_message_system_message() {
        let json = r#"{
            "id": "2048",
            "token": "room_xyz",
            "actor_type": "users",
            "actor_id": "system",
            "actor_display_name": "System",
            "timestamp": "1718000050",
            "message": "Alice joined the conversation",
            "system_message": "user_joined"
        }"#;

        let msg: ChatMessage = serde_json::from_str(json).expect("valid chat message json");
        assert_eq!(msg.id, 2048);
        assert_eq!(msg.token, "room_xyz");
        assert_eq!(msg.system_message, "user_joined");
        assert!(msg.is_system_message());
    }
}
