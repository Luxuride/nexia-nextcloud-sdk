use std::error::Error;
use std::fmt;

/// An OCS envelope that carries no readable verdict.
///
/// Nextcloud documents `meta.statuscode` as a number, serialises it as a
/// string, and sends an empty string on some endpoints. Neither the code nor
/// the `status` text then says whether the call succeeded, and guessing would
/// turn a malformed answer into a silent success, so it is reported as a
/// decode failure instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatusCodeError {
    value: String,
}

impl StatusCodeError {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl fmt::Display for StatusCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OCS statuscode `{}` is not a recognised verdict",
            self.value
        )
    }
}

impl Error for StatusCodeError {}

#[cfg(test)]
mod tests {
    use super::StatusCodeError;

    #[test]
    fn display_names_the_offending_value() {
        let rendered = StatusCodeError::new("").to_string();
        assert_eq!(rendered, "OCS statuscode `` is not a recognised verdict");
    }
}
