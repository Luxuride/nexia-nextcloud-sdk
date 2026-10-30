//! Wire format deserialization helpers for the Activity API.

use serde::Deserialize;
use serde::de::{self, Deserializer};

/// Deserializes a `u64` from either a numeric value or a JSON string.
pub(crate) fn lossy_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Num(u64),
        Str(String),
    }

    match Wire::deserialize(deserializer)? {
        Wire::Num(n) => Ok(n),
        Wire::Str(s) => s.trim().parse::<u64>().map_err(de::Error::custom),
    }
}

/// Deserializes an `Option<u64>` from a number, string, null, or empty string.
pub(crate) fn lossy_opt_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Num(u64),
        Str(String),
        None,
    }

    match Option::<Wire>::deserialize(deserializer)? {
        Some(Wire::Num(n)) => Ok(Some(n)),
        Some(Wire::Str(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed.parse::<u64>().map(Some).map_err(de::Error::custom)
            }
        }
        Some(Wire::None) | None => Ok(None),
    }
}
