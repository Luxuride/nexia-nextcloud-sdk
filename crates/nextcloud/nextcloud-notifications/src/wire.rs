//! Wire deserialization utilities for Nextcloud Notifications JSON payloads.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

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

/// Deserializes a string from either a JSON string, a number, or null.
pub(crate) fn lossy_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrNumVisitor;

    impl Visitor<'_> for StringOrNumVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string, number, or null")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(v.to_owned())
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
            Ok(v.to_string())
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }
    }

    deserializer.deserialize_any(StringOrNumVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct LossyTest {
        #[serde(deserialize_with = "lossy_u64")]
        id: u64,
        #[serde(deserialize_with = "lossy_string")]
        val: String,
    }

    #[test]
    fn parses_numeric_and_string_values() {
        let from_num: LossyTest = serde_json::from_str(r#"{"id": 42, "val": 123}"#).unwrap();
        assert_eq!(from_num.id, 42);
        assert_eq!(from_num.val, "123");

        let from_str: LossyTest = serde_json::from_str(r#"{"id": "42", "val": "hello"}"#).unwrap();
        assert_eq!(from_str.id, 42);
        assert_eq!(from_str.val, "hello");
    }
}
