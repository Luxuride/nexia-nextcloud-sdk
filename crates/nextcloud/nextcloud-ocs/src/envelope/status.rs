use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Serialize, Serializer};

/// Verdict an OCS envelope's `meta` block carries.
///
/// Nextcloud reports the verdict twice: as `meta.status` (`"ok"`/`"failure"`)
/// and as `meta.statuscode`. The numeric code is the authoritative one, since
/// it distinguishes causes (`997` app not installed, `998` not allowed, `999`
/// invalid) that the two-valued status cannot express.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OcsStatus {
    /// The call succeeded.
    Ok,
    /// The server refused the call.
    Failure,
}

impl OcsStatus {
    /// Maps an OCS `statuscode` to its verdict.
    ///
    /// `100` and `200` mean success; every other code is a failure.
    pub const fn from_statuscode(code: u16) -> Self {
        match code {
            100 | 200 => Self::Ok,
            _ => Self::Failure,
        }
    }

    /// Parses the `meta.status` text, matched case-insensitively.
    ///
    /// An unrecognised value yields `None` rather than a guessed verdict, so a
    /// renamed status surfaces as a decode failure instead of a wrong answer.
    pub fn from_status_text(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ok" => Some(Self::Ok),
            "failure" => Some(Self::Failure),
            _ => None,
        }
    }

    /// Wire representation used in `meta.status`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failure => "failure",
        }
    }

    /// Whether this verdict is [`OcsStatus::Ok`].
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

impl fmt::Display for OcsStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for OcsStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> de::Deserialize<'de> for OcsStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(StatusVisitor)
    }
}

struct StatusVisitor;

impl Visitor<'_> for StatusVisitor {
    type Value = OcsStatus;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("`ok` or `failure`")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        OcsStatus::from_status_text(value)
            .ok_or_else(|| E::invalid_value(de::Unexpected::Str(value), &self))
    }
}

#[cfg(test)]
mod tests {
    use super::OcsStatus;

    #[test]
    fn statuscodes_map_to_a_verdict() {
        assert_eq!(OcsStatus::from_statuscode(100), OcsStatus::Ok);
        assert_eq!(OcsStatus::from_statuscode(200), OcsStatus::Ok);
        assert_eq!(OcsStatus::from_statuscode(997), OcsStatus::Failure);
        assert_eq!(OcsStatus::from_statuscode(998), OcsStatus::Failure);
        assert_eq!(OcsStatus::from_statuscode(999), OcsStatus::Failure);
        assert!(OcsStatus::from_statuscode(200).is_ok());
    }

    #[test]
    fn status_text_is_matched_case_insensitively() {
        assert_eq!(OcsStatus::from_status_text("OK"), Some(OcsStatus::Ok));
        assert_eq!(
            OcsStatus::from_status_text("Failure"),
            Some(OcsStatus::Failure)
        );
        assert_eq!(OcsStatus::from_status_text("maybe"), None);
    }

    #[test]
    fn deserializes_from_status_text() {
        let status: OcsStatus = serde_json::from_str("\"ok\"").unwrap();
        assert_eq!(status, OcsStatus::Ok);
        assert!(serde_json::from_str::<OcsStatus>("\"unknown\"").is_err());
    }

    #[test]
    fn serializes_back_to_status_text() {
        assert_eq!(
            serde_json::to_string(&OcsStatus::Failure).unwrap(),
            "\"failure\""
        );
    }
}
