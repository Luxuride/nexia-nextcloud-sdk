use std::fmt;

use serde::de::{self, Deserializer, Unexpected, Visitor};

/// Reads a `meta` field that Nextcloud sends either as a JSON string or as a
/// bare number, always yielding a string.
///
/// The documented envelope declares `statuscode`, `totalitems` and
/// `itemsperpage` as strings, but the server emits numbers for some of them
/// and an empty string for others depending on the endpoint and version.
/// Normalising here keeps [`Meta`](super::Meta) free of mixed types.
pub(super) fn lossy_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    deserializer.deserialize_any(TextVisitor)
}

struct TextVisitor;

impl<'de> Visitor<'de> for TextVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string, a number, or null")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(value.to_owned())
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(value)
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(value.to_string())
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(value.to_string())
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(value.to_string())
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(value.to_string())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(String::new())
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(String::new())
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
        Err(E::invalid_value(Unexpected::Bytes(value), &self))
    }
}
