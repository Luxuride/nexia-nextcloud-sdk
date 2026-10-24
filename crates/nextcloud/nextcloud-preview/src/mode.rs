//! Preview scaling modes for Nextcloud thumbnails.

use std::fmt;

/// Scaling mode for generating previews.
///
/// Controls how Nextcloud resizes or crops the image when generating a thumbnail
/// with dimensions that do not match the original aspect ratio.
///
/// See the Nextcloud preview endpoint parameters for details.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PreviewMode {
    /// Scale image to fill dimensions and crop excess to fit the requested box.
    Cover,
    /// Scale image to fit within dimensions without cropping (preserving aspect ratio).
    Fill,
}

impl PreviewMode {
    /// Returns the wire parameter string for this preview mode.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Fill => "fill",
        }
    }
}

impl fmt::Display for PreviewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::PreviewMode;

    #[test]
    fn as_str_returns_expected_wire_values() {
        assert_eq!(PreviewMode::Cover.as_str(), "cover");
        assert_eq!(PreviewMode::Fill.as_str(), "fill");
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(PreviewMode::Cover.to_string(), "cover");
        assert_eq!(PreviewMode::Fill.to_string(), "fill");
    }

    #[test]
    fn equality_and_copy() {
        let mode = PreviewMode::Cover;
        let copied = mode;
        assert_eq!(mode, copied);
        assert_ne!(mode, PreviewMode::Fill);
    }
}
