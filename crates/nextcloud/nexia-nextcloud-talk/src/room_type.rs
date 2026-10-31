//! Conversation room type enumeration for Nextcloud Talk.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The type of a Nextcloud Talk conversation.
///
/// Nextcloud Talk identifies conversation types by numeric codes:
/// - One-to-one conversation (`1`)
/// - Group conversation (`2`)
/// - Public link conversation (`3`)
///
/// Talk also reports codes this SDK does not model (a changelog conversation is `4`, a note-to-self
/// conversation is `6`); those are carried through as [`RoomType::Unknown`] rather than failing the
/// listing they arrived in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoomType {
    /// One-to-one direct conversation between two users (`1`).
    OneToOne = 1,
    /// Group conversation (`2`).
    Group = 2,
    /// Public conversation accessible via public link (`3`).
    Public = 3,
    /// A conversation type this SDK does not recognise; the wire code is preserved.
    Unknown(u8),
}

impl RoomType {
    /// Numeric code sent over the wire.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::OneToOne => 1,
            Self::Group => 2,
            Self::Public => 3,
            Self::Unknown(code) => code,
        }
    }

    /// Maps any wire code to a [`RoomType`], preserving codes this SDK does not know.
    pub const fn from_u8(code: u8) -> Self {
        match code {
            1 => Self::OneToOne,
            2 => Self::Group,
            3 => Self::Public,
            other => Self::Unknown(other),
        }
    }

    /// `false` when this is a code this SDK does not recognise.
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// Human-readable label for this room type.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneToOne => "one-to-one",
            Self::Group => "group",
            Self::Public => "public",
            Self::Unknown(_) => "unknown",
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
                f.write_str("a Nextcloud Talk room type code (integer or string)")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<RoomType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("room type code {v} out of range")))?;
                Ok(RoomType::from_u8(code))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<RoomType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("room type code {v} out of range")))?;
                Ok(RoomType::from_u8(code))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<RoomType, E> {
                let code: u8 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid room type string '{v}'")))?;
                Ok(RoomType::from_u8(code))
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

impl From<RoomType> for u8 {
    fn from(room_type: RoomType) -> Self {
        room_type.as_u8()
    }
}

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
        assert_eq!(RoomType::from_u8(1), RoomType::OneToOne);
        assert_eq!(RoomType::from_u8(2), RoomType::Group);
        assert_eq!(RoomType::from_u8(3), RoomType::Public);

        assert_eq!(u8::from(RoomType::OneToOne), 1);
        assert_eq!(u8::from(RoomType::Group), 2);
        assert_eq!(u8::from(RoomType::Public), 3);
    }

    /// Talk reports a changelog conversation as `4` and a note-to-self conversation as `6`; a
    /// listing of ordinary rooms must not be lost because of them.
    #[test]
    fn unknown_room_types_do_not_fail_the_listing() {
        for code in [4, 6, 99] {
            let room_type = RoomType::from_u8(code);
            assert_eq!(room_type, RoomType::Unknown(code));
            assert!(!room_type.is_known());
            assert_eq!(room_type.as_u8(), code);
            assert_eq!(room_type.as_str(), "unknown");
        }
        assert!(RoomType::from_u8(2).is_known());
    }

    #[test]
    fn room_type_serde_round_trip() {
        let serialized = serde_json::to_string(&RoomType::Group).expect("serialize");
        assert_eq!(serialized, "2");
        let deserialized: RoomType = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, RoomType::Group);

        let from_str: RoomType = serde_json::from_str("\"3\"").expect("deserialize from string");
        assert_eq!(from_str, RoomType::Public);

        let unknown: RoomType = serde_json::from_str("6").expect("deserialize unknown");
        assert_eq!(unknown, RoomType::Unknown(6));
    }
}
