//! Conversation room type enumeration for Nextcloud Talk.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The type of a Nextcloud Talk conversation.
///
/// Nextcloud Talk identifies conversation types by numeric codes:
/// - One-to-one conversation (`1`)
/// - Group conversation (`2`)
/// - Public link conversation (`3`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoomType {
    /// One-to-one direct conversation between two users (`1`).
    OneToOne = 1,
    /// Group conversation (`2`).
    Group = 2,
    /// Public conversation accessible via public link (`3`).
    Public = 3,
}

impl RoomType {
    /// Numeric code sent over the wire.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Maps a wire code to a [`RoomType`], returning `None` for unknown codes.
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::OneToOne),
            2 => Some(Self::Group),
            3 => Some(Self::Public),
            _ => None,
        }
    }

    /// Human-readable label for this room type.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneToOne => "one-to-one",
            Self::Group => "group",
            Self::Public => "public",
        }
    }
}

impl Serialize for RoomType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for RoomType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RoomTypeVisitor;

        impl serde::de::Visitor<'_> for RoomTypeVisitor {
            type Value = RoomType;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a valid Nextcloud Talk room type code (1, 2, or 3)")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<RoomType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("room type code {v} out of range")))?;
                RoomType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown room type code {code}")))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<RoomType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("room type code {v} out of range")))?;
                RoomType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown room type code {code}")))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<RoomType, E> {
                let code: u8 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid room type string '{v}'")))?;
                RoomType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown room type code {code}")))
            }
        }

        deserializer.deserialize_any(RoomTypeVisitor)
    }
}

impl fmt::Display for RoomType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<u8> for RoomType {
    type Error = UnknownRoomTypeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_u8(value).ok_or(UnknownRoomTypeError(value))
    }
}

impl From<RoomType> for u8 {
    fn from(room_type: RoomType) -> Self {
        room_type.as_u8()
    }
}

/// Error returned when converting an unrecognized integer into a [`RoomType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownRoomTypeError(u8);

impl UnknownRoomTypeError {
    /// The unrecognised numeric code.
    pub const fn code(self) -> u8 {
        self.0
    }
}

impl fmt::Display for UnknownRoomTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown room type code: {}", self.0)
    }
}

impl std::error::Error for UnknownRoomTypeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_type_display_and_as_str() {
        assert_eq!(RoomType::OneToOne.as_str(), "one-to-one");
        assert_eq!(RoomType::Group.as_str(), "group");
        assert_eq!(RoomType::Public.as_str(), "public");
        assert_eq!(RoomType::OneToOne.to_string(), "one-to-one");
    }

    #[test]
    fn room_type_conversion_u8() {
        assert_eq!(RoomType::from_u8(1), Some(RoomType::OneToOne));
        assert_eq!(RoomType::from_u8(2), Some(RoomType::Group));
        assert_eq!(RoomType::from_u8(3), Some(RoomType::Public));
        assert_eq!(RoomType::from_u8(99), None);

        assert_eq!(RoomType::try_from(1), Ok(RoomType::OneToOne));
        assert_eq!(RoomType::try_from(2), Ok(RoomType::Group));
        assert_eq!(RoomType::try_from(3), Ok(RoomType::Public));
        let err = RoomType::try_from(99).unwrap_err();
        assert_eq!(err.code(), 99);
        assert_eq!(err.to_string(), "unknown room type code: 99");

        assert_eq!(u8::from(RoomType::OneToOne), 1);
        assert_eq!(u8::from(RoomType::Group), 2);
        assert_eq!(u8::from(RoomType::Public), 3);
    }

    #[test]
    fn room_type_serde_round_trip() {
        let serialized = serde_json::to_string(&RoomType::Group).expect("serialize");
        assert_eq!(serialized, "2");
        let deserialized: RoomType = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, RoomType::Group);

        let from_str: RoomType = serde_json::from_str("\"3\"").expect("deserialize from string");
        assert_eq!(from_str, RoomType::Public);
    }
}
