//! Wire deserialization utilities for Nextcloud Talk JSON payloads.

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

/// Deserializes an `i64` from either a numeric value or a JSON string.
pub(crate) fn lossy_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Num(i64),
        Str(String),
    }

    match Wire::deserialize(deserializer)? {
        Wire::Num(n) => Ok(n),
        Wire::Str(s) => s.trim().parse::<i64>().map_err(de::Error::custom),
    }
}

/// Deserializes an `Option<u32>` from a number, string, null, or empty string.
pub(crate) fn lossy_opt_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Num(u32),
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
                trimmed.parse::<u32>().map(Some).map_err(de::Error::custom)
            }
        }
        Some(Wire::None) | None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestItem {
        #[serde(deserialize_with = "lossy_u64")]
        id: u64,
        #[serde(deserialize_with = "lossy_i64")]
        ts: i64,
        #[serde(default, deserialize_with = "lossy_opt_u32")]
        count: Option<u32>,
    }

    #[test]
    fn lossy_numeric_deserialization() {
        let json = r#"{"id": "42", "ts": 123456789, "count": "10"}"#;
        let item: TestItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, 42);
        assert_eq!(item.ts, 123456789);
        assert_eq!(item.count, Some(10));

        let json2 = r#"{"id": 100, "ts": "987654321", "count": null}"#;
        let item2: TestItem = serde_json::from_str(json2).unwrap();
        assert_eq!(item2.id, 100);
        assert_eq!(item2.ts, 987654321);
        assert_eq!(item2.count, None);
    }
}
