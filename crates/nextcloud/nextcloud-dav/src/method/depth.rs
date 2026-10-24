/// Header carrying a WebDAV `Depth` value.
pub const DEPTH_HEADER: &str = "Depth";

/// How far a `PROPFIND` reaches below the addressed resource (RFC 4918
/// §9.1).
///
/// The variants are the three tokens the specification defines; the server
/// decides which of them it honours, and Nextcloud refuses `Infinity` for
/// most trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Depth {
    /// The resource itself.
    Zero,
    /// The resource and its immediate children.
    One,
    /// The whole subtree.
    Infinity,
}

impl Depth {
    /// Wire representation for the [`DEPTH_HEADER`] value.
    pub const fn as_header_value(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
            Self::Infinity => "infinity",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEPTH_HEADER, Depth};

    #[test]
    fn header_values_use_the_specification_tokens() {
        assert_eq!(Depth::Zero.as_header_value(), "0");
        assert_eq!(Depth::One.as_header_value(), "1");
        assert_eq!(Depth::Infinity.as_header_value(), "infinity");
    }

    #[test]
    fn header_name_keeps_its_canonical_spelling() {
        assert_eq!(DEPTH_HEADER, "Depth");
    }
}
