use serde::Deserialize;
use serde::de::{self, Deserializer};

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

/// Deserializes an `f64` from either a numeric value or a JSON string.
pub(crate) fn lossy_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Num(f64),
        Str(String),
    }

    match Wire::deserialize(deserializer)? {
        Wire::Num(n) => Ok(n),
        Wire::Str(s) => s.trim().parse::<f64>().map_err(de::Error::custom),
    }
}

/// Deserializes a `bool` from either a boolean, integer (0/1), or JSON string ("true"/"false"/"1"/"0").
pub(crate) fn lossy_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Bool(bool),
        Num(i64),
        Str(String),
    }

    match Wire::deserialize(deserializer)? {
        Wire::Bool(b) => Ok(b),
        Wire::Num(n) => Ok(n != 0),
        Wire::Str(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" | "" => Ok(false),
            other => Err(de::Error::custom(format!(
                "invalid boolean string: {other}"
            ))),
        },
    }
}
