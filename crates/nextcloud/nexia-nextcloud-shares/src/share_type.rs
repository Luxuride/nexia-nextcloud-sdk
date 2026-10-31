use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The recipient type of a Nextcloud share.
///
/// Nextcloud identifies share targets by integer codes on the wire. A server may report a code this
/// SDK does not know (a newer Nextcloud, or an app that registers its own share type), which is
/// carried through as [`ShareType::Unknown`] instead of failing the whole listing.
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
    /// A share type this SDK does not recognise; the wire code is preserved.
    Unknown(u8),
}

impl ShareType {
    /// Numeric code sent over the wire.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::User => 0,
            Self::Group => 1,
            Self::PublicLink => 3,
            Self::Email => 4,
            Self::Remote => 6,
            Self::Circle => 7,
            Self::Room => 10,
            Self::Unknown(code) => code,
        }
    }

    /// Maps any wire code to a [`ShareType`], preserving codes this SDK does not know.
    pub const fn from_u8(code: u8) -> Self {
        match code {
            0 => Self::User,
            1 => Self::Group,
            3 => Self::PublicLink,
            4 => Self::Email,
            6 => Self::Remote,
            7 => Self::Circle,
            10 => Self::Room,
            other => Self::Unknown(other),
        }
    }

    /// `false` when this is a code this SDK does not recognise.
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown(_))
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
            Self::Unknown(_) => "unknown",
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
                f.write_str("a Nextcloud share type code (integer or string)")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<ShareType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("share type code {v} out of range")))?;
                Ok(ShareType::from_u8(code))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<ShareType, E> {
                let code = u8::try_from(v)
                    .map_err(|_| E::custom(format!("share type code {v} out of range")))?;
                Ok(ShareType::from_u8(code))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<ShareType, E> {
                let code: u8 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid share type string '{v}'")))?;
                Ok(ShareType::from_u8(code))
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

impl From<ShareType> for u8 {
    fn from(share_type: ShareType) -> Self {
        share_type.as_u8()
    }
}

#[cfg(test)]
mod tests {
    use super::ShareType;

    #[test]
    fn maps_all_expected_codes() {
        assert_eq!(ShareType::from_u8(0), ShareType::User);
        assert_eq!(ShareType::from_u8(1), ShareType::Group);
        assert_eq!(ShareType::from_u8(3), ShareType::PublicLink);
        assert_eq!(ShareType::from_u8(4), ShareType::Email);
        assert_eq!(ShareType::from_u8(6), ShareType::Remote);
        assert_eq!(ShareType::from_u8(7), ShareType::Circle);
        assert_eq!(ShareType::from_u8(10), ShareType::Room);
    }

    /// A share type this SDK does not know must not fail the listing it arrived in.
    #[test]
    fn unknown_codes_are_preserved() {
        for code in [2, 5, 9, 12, 255] {
            let share_type = ShareType::from_u8(code);
            assert_eq!(share_type, ShareType::Unknown(code));
            assert!(!share_type.is_known());
            assert_eq!(share_type.as_u8(), code);
            assert_eq!(share_type.as_str(), "unknown");
        }
        assert!(ShareType::from_u8(0).is_known());
    }

    #[test]
    fn as_u8_round_trips() {
        for code in [0, 1, 3, 4, 6, 7, 10, 2, 12, 255] {
            let st = ShareType::from_u8(code);
            assert_eq!(st.as_u8(), code);
            assert_eq!(u8::from(st), code);
        }
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
    fn serde_json_keeps_unknown_codes_and_rejects_invalid_values() {
        assert_eq!(
            serde_json::from_str::<ShareType>("12").unwrap(),
            ShareType::Unknown(12)
        );
        assert_eq!(
            serde_json::to_string(&ShareType::Unknown(12)).unwrap(),
            "12"
        );
        assert!(serde_json::from_str::<ShareType>("\"invalid\"").is_err());
        assert!(serde_json::from_str::<ShareType>("300").is_err());
    }

    #[test]
    fn labels_and_display() {
        assert_eq!(ShareType::User.as_str(), "user");
        assert_eq!(ShareType::User.to_string(), "user");
        assert_eq!(ShareType::PublicLink.as_str(), "public_link");
    }
}
