use std::fmt;

/// Serialisation format to request from an OCS endpoint.
///
/// OCS answers in XML unless asked otherwise, so a JSON client must always
/// send `format=json` — this enum exists to keep that requirement visible
/// instead of burying it in a string literal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OcsFormat {
    /// `format=json`
    #[default]
    Json,
    /// `format=xml`, OCS's default when the parameter is absent.
    Xml,
}

impl OcsFormat {
    /// Value of the `format` query parameter.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }

    /// Name of the parameter carrying the format.
    pub const PARAM: &'static str = "format";
}

impl fmt::Display for OcsFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::OcsFormat;

    #[test]
    fn json_is_the_default() {
        assert_eq!(OcsFormat::default(), OcsFormat::Json);
        assert_eq!(OcsFormat::Json.to_string(), "json");
        assert_eq!(OcsFormat::Xml.to_string(), "xml");
    }
}
