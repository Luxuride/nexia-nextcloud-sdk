//! Conversation room model for Nextcloud Talk.

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::room_type::RoomType;

/// A Nextcloud Talk conversation or room.
///
/// Models metadata about a conversation such as its unique token, name,
/// conversation type, password protection state, and participant count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Room {
    /// Unique alphanumeric identifier for the conversation.
    pub token: String,
    /// Conversation name or display name.
    pub name: String,
    /// Type of conversation (one-to-one, group, or public link).
    pub room_type: RoomType,
    /// Optional conversation description.
    pub description: Option<String>,
    /// Whether the conversation is protected with a password.
    pub has_password: bool,
    /// Number of participants in the conversation, if reported.
    pub participant_count: Option<u32>,
}

impl Room {
    /// Creates a new [`Room`] descriptor.
    pub fn new(
        token: impl Into<String>,
        name: impl Into<String>,
        room_type: RoomType,
        description: Option<String>,
        has_password: bool,
        participant_count: Option<u32>,
    ) -> Self {
        Self {
            token: token.into(),
            name: name.into(),
            room_type,
            description,
            has_password,
            participant_count,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomWire {
    token: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default, alias = "roomName")]
    room_name: Option<String>,
    #[serde(alias = "room_type", alias = "roomType", alias = "type")]
    room_type: RoomType,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, alias = "has_password")]
    has_password: bool,
    #[serde(
        default,
        alias = "numParticipants",
        alias = "participant_count",
        deserialize_with = "crate::wire::lossy_opt_u32"
    )]
    participant_count: Option<u32>,
}

impl From<RoomWire> for Room {
    fn from(wire: RoomWire) -> Self {
        let name = wire
            .name
            .filter(|s| !s.trim().is_empty())
            .or_else(|| wire.display_name.filter(|s| !s.trim().is_empty()))
            .or_else(|| wire.room_name.filter(|s| !s.trim().is_empty()))
            .unwrap_or_default();

        let description = wire.description.filter(|s| !s.trim().is_empty());

        Self {
            token: wire.token,
            name,
            room_type: wire.room_type,
            description,
            has_password: wire.has_password,
            participant_count: wire.participant_count,
        }
    }
}

impl<'de> Deserialize<'de> for Room {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        RoomWire::deserialize(deserializer).map(Self::from)
    }
}

impl Serialize for Room {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Room", 6)?;
        state.serialize_field("token", &self.token)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("room_type", &self.room_type)?;
        state.serialize_field("description", &self.description)?;
        state.serialize_field("has_password", &self.has_password)?;
        state.serialize_field("participant_count", &self.participant_count)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_deserialization_from_talk_v4_json() {
        let json = r#"{
            "token": "d7a8f9b2",
            "name": "General Chat",
            "displayName": "General Chat",
            "type": 2,
            "description": "Company wide discussion",
            "hasPassword": false,
            "participantCount": 15
        }"#;

        let room: Room = serde_json::from_str(json).expect("valid room json");
        assert_eq!(room.token, "d7a8f9b2");
        assert_eq!(room.name, "General Chat");
        assert_eq!(room.room_type, RoomType::Group);
        assert_eq!(room.description.as_deref(), Some("Company wide discussion"));
        assert!(!room.has_password);
        assert_eq!(room.participant_count, Some(15));
    }

    #[test]
    fn room_deserialization_fallback_display_name_and_room_type() {
        let json = r#"{
            "token": "x1y2z3",
            "displayName": "Alice & Bob",
            "roomType": 1,
            "hasPassword": true
        }"#;

        let room: Room = serde_json::from_str(json).expect("valid room json");
        assert_eq!(room.token, "x1y2z3");
        assert_eq!(room.name, "Alice & Bob");
        assert_eq!(room.room_type, RoomType::OneToOne);
        assert_eq!(room.description, None);
        assert!(room.has_password);
        assert_eq!(room.participant_count, None);
    }

    #[test]
    fn room_serialization_round_trip() {
        let room = Room::new(
            "public123",
            "Support Room",
            RoomType::Public,
            Some("Public assistance".into()),
            false,
            Some(42),
        );

        let json = serde_json::to_string(&room).expect("serialized");
        let parsed: Room = serde_json::from_str(&json).expect("deserialized");
        assert_eq!(room, parsed);
    }
}
