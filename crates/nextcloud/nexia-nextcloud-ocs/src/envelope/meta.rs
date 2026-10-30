use serde::{Deserialize, Serialize};

use super::OcsStatus;
use super::text::lossy_string;

/// The `meta` block of an OCS envelope: the verdict plus pagination hints.
///
/// Every field is held as a string because that is what Nextcloud documents
/// (`statuscode`, `totalitems` and `itemsperpage` are declared as strings) —
/// yet the server sends bare numbers for several of them, and an empty string
/// where a count does not apply. Deserialisation accepts both forms and
/// normalises to text; [`Meta::code`] parses the numeric verdict on demand.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Meta {
    #[serde(default, deserialize_with = "lossy_string")]
    status: String,
    #[serde(default, deserialize_with = "lossy_string")]
    statuscode: String,
    #[serde(default, deserialize_with = "lossy_string")]
    message: String,
    #[serde(default, deserialize_with = "lossy_string")]
    totalitems: String,
    #[serde(default, deserialize_with = "lossy_string")]
    itemsperpage: String,
}

impl Meta {
    /// Verdict text as sent, `"ok"` or `"failure"`.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// `statuscode` as sent, usually `"100"`, `"200"` or a failure code.
    pub fn statuscode(&self) -> &str {
        &self.statuscode
    }

    /// Human-readable message, often carrying the failure reason.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Total number of items across all pages, when the endpoint paginates.
    pub fn totalitems(&self) -> &str {
        &self.totalitems
    }

    /// Items per page, when the endpoint paginates.
    pub fn itemsperpage(&self) -> &str {
        &self.itemsperpage
    }

    /// `statuscode` parsed as a number, `None` when absent or not a number.
    pub fn code(&self) -> Option<u16> {
        self.statuscode.trim().parse().ok()
    }

    /// Verdict from `statuscode`, falling back to the `status` text for
    /// envelopes that omit the numeric code.
    pub fn verdict(&self) -> Option<OcsStatus> {
        self.code()
            .map(OcsStatus::from_statuscode)
            .or_else(|| OcsStatus::from_status_text(&self.status))
    }

    /// Whether the envelope reports success.
    ///
    /// An envelope that carries neither a readable code nor a readable status
    /// is not a success: the absence of a verdict must not read as one.
    pub fn is_ok(&self) -> bool {
        self.verdict().is_some_and(OcsStatus::is_ok)
    }
}

#[cfg(test)]
mod tests {
    use super::Meta;
    use crate::envelope::OcsStatus;

    #[test]
    fn accepts_string_fields() {
        let meta: Meta = serde_json::from_str(
            r#"{"status":"ok","statuscode":200,"message":"","totalitems":"","itemsperpage":""}"#,
        )
        .unwrap();
        assert_eq!(meta.code(), Some(200));
        assert!(meta.is_ok());
    }

    #[test]
    fn accepts_numeric_fields() {
        let meta: Meta =
            serde_json::from_str(r#"{"status":"ok","statuscode":200,"totalitems":3}"#).unwrap();
        assert_eq!(meta.statuscode(), "200");
        assert_eq!(meta.totalitems(), "3");
        assert_eq!(meta.verdict(), Some(OcsStatus::Ok));
    }

    #[test]
    fn missing_fields_default_to_empty() {
        let meta: Meta = serde_json::from_str(r#"{"status":"failure"}"#).unwrap();
        assert_eq!(meta.statuscode(), "");
        assert_eq!(meta.code(), None);
        assert_eq!(meta.verdict(), Some(OcsStatus::Failure));
        assert!(!meta.is_ok());
    }

    #[test]
    fn unknown_verdict_is_not_success() {
        let meta: Meta = serde_json::from_str(r#"{"status":"","statuscode":""}"#).unwrap();
        assert_eq!(meta.verdict(), None);
        assert!(!meta.is_ok());
    }
}
