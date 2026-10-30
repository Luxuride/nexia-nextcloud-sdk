use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The recipient type of a Nextcloud share.
///
/// Nextcloud identifies share targets by integer codes on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ShareType {
    /// Share with an individual Nextcloud user (`0`).
    User = 0,
    /// Share with a user group (`1`).
    Group = 1,
    /// Share via a public link (`3`).
    PublicLink = 3,
    /// Share via email invitation (`4`).
    Email = 4,
    /// Federated cloud share to a remote Nextcloud instance (`6`).
    Remote = 6,
    /// Share with a Nextcloud Circle (`7`).
    Circle = 7,
    /// Share with a Talk conversation or room (`10`).
    Room = 10,
}

impl ShareType {
    /// Numeric code sent over the wire.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Maps a wire code to a [`ShareType`], returning `None` for unknown codes.
    pub const fn from_u8(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::User),
            1 => Some(Self::Group),
            3 => Some(Self::PublicLink),
            4 => Some(Self::Email),
            6 => Some(Self::Remote),
            7 => Some(Self::Circle),
            10 => Some(Self::Room),
            _ => None,
        }
    }

    /// Human-readable label for this share type.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Group => "group",
            Self::PublicLink => "public_link",
            Self::Email => "email",
            Self::Remote => "remote",
            Self::Circle => "circle",
            Self::Room => "room",
        }
    }
}

impl Serialize for ShareType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for ShareType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ShareTypeVisitor;

        impl serde::de::Visitor<'_> for ShareTypeVisitor {
            type Value = ShareType;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a valid Nextcloud share type code (integer or string)")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ShareType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("share type code {v} out of range")))?;
                ShareType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown share type code {code}")))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<ShareType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("share type code {v} out of range")))?;
                ShareType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown share type code {code}")))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ShareType, E> {
                let code: u8 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid share type string '{v}'")))?;
                ShareType::from_u8(code)
                    .ok_or_else(|| E::custom(format!("unknown share type code {code}")))
            }
        }

        deserializer.deserialize_any(ShareTypeVisitor)
    }
}

impl fmt::Display for ShareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<u8> for ShareType {
    type Error = UnknownShareTypeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_u8(value).ok_or(UnknownShareTypeError(value))
    }
}

impl From<ShareType> for u8 {
    fn from(share_type: ShareType) -> Self {
        share_type.as_u8()
    }
}

/// Error returned when converting an unrecognized integer into a [`ShareType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownShareTypeError(u8);

impl UnknownShareTypeError {
    /// The unrecognised numeric code.
    pub const fn code(self) -> u8 {
        self.0
    }
}

impl fmt::Display for UnknownShareTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown share type code: {}", self.0)
    }
}

impl std::error::Error for UnknownShareTypeError {}

#[cfg(test)]
mod tests {
    use super::{ShareType, UnknownShareTypeError};

    #[test]
    fn maps_all_expected_codes() {
        assert_eq!(ShareType::from_u8(0), Some(ShareType::User));
        assert_eq!(ShareType::from_u8(1), Some(ShareType::Group));
        assert_eq!(ShareType::from_u8(3), Some(ShareType::PublicLink));
        assert_eq!(ShareType::from_u8(4), Some(ShareType::Email));
        assert_eq!(ShareType::from_u8(6), Some(ShareType::Remote));
        assert_eq!(ShareType::from_u8(7), Some(ShareType::Circle));
        assert_eq!(ShareType::from_u8(10), Some(ShareType::Room));
        assert_eq!(ShareType::from_u8(2), None);
        assert_eq!(ShareType::from_u8(99), None);
    }

    #[test]
    fn as_u8_round_trips() {
        for code in [0, 1, 3, 4, 6, 7, 10] {
            let st = ShareType::from_u8(code).unwrap();
            assert_eq!(st.as_u8(), code);
            assert_eq!(u8::from(st), code);
            assert_eq!(ShareType::try_from(code), Ok(st));
        }
        assert_eq!(ShareType::try_from(255), Err(UnknownShareTypeError(255)));
    }

    #[test]
    fn serde_json_round_trips_numbers() {
        let json = serde_json::to_string(&ShareType::PublicLink).unwrap();
        assert_eq!(json, "3");
        let parsed: ShareType = serde_json::from_str("3").unwrap();
        assert_eq!(parsed, ShareType::PublicLink);
    }

    #[test]
    fn serde_json_deserializes_from_string() {
        let parsed: ShareType = serde_json::from_str("\"0\"").unwrap();
        assert_eq!(parsed, ShareType::User);
    }

    #[test]
    fn serde_json_rejects_invalid_values() {
        assert!(serde_json::from_str::<ShareType>("99").is_err());
        assert!(serde_json::from_str::<ShareType>("\"invalid\"").is_err());
    }

    #[test]
    fn labels_and_display() {
        assert_eq!(ShareType::User.as_str(), "user");
        assert_eq!(ShareType::User.to_string(), "user");
        assert_eq!(ShareType::PublicLink.as_str(), "public_link");
    }
}
