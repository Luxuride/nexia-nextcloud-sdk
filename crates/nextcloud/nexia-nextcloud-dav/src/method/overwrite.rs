/// Header specifying whether to overwrite a destination resource (RFC 4918 §10.6).
pub const OVERWRITE_HEADER: &str = "Overwrite";

/// Behaviour when the destination resource already exists (RFC 4918 §10.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Overwrite {
    /// Overwrite the target if it already exists (`"T"`).
    Allow,
    /// Do not overwrite the target if it already exists (`"F"`).
    Forbidden,
}

impl Overwrite {
    /// Wire representation for the [`OVERWRITE_HEADER`] value.
    pub const fn as_header_value(self) -> &'static str {
        match self {
            Self::Allow => "T",
            Self::Forbidden => "F",
        }
    }
}

impl From<bool> for Overwrite {
    fn from(value: bool) -> Self {
        if value { Self::Allow } else { Self::Forbidden }
    }
}

#[cfg(test)]
mod tests {
    use super::{OVERWRITE_HEADER, Overwrite};

    #[test]
    fn header_values_use_the_specification_tokens() {
        assert_eq!(Overwrite::Allow.as_header_value(), "T");
        assert_eq!(Overwrite::Forbidden.as_header_value(), "F");
    }

    #[test]
    fn header_name_keeps_its_canonical_spelling() {
        assert_eq!(OVERWRITE_HEADER, "Overwrite");
    }

    #[test]
    fn boolean_conversion_matches_specification() {
        assert_eq!(Overwrite::from(true), Overwrite::Allow);
        assert_eq!(Overwrite::from(false), Overwrite::Forbidden);
    }
}
