//! Options and builder for Nextcloud preview requests.

use crate::mode::PreviewMode;

/// Parameters for generating a thumbnail or preview image.
///
/// Corresponds to the query parameters accepted by Nextcloud's preview endpoint:
/// - `x`: Width in pixels
/// - `y`: Height in pixels
/// - `a`: Whether to keep aspect ratio (`1` or `0`)
/// - `mode`: Scaling mode (`"cover"` or `"fill"`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewOptions {
    width: Option<u32>,
    height: Option<u32>,
    keep_aspect_ratio: Option<bool>,
    mode: Option<PreviewMode>,
}

impl PreviewOptions {
    /// Creates an empty set of preview options.
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            keep_aspect_ratio: None,
            mode: None,
        }
    }

    /// Sets the desired preview width in pixels (`x` parameter).
    #[must_use]
    pub const fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets the desired preview height in pixels (`y` parameter).
    #[must_use]
    pub const fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// Sets both width and height in pixels (`x` and `y` parameters).
    #[must_use]
    pub const fn dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Specifies whether to keep the image aspect ratio (`a` parameter).
    ///
    /// When `true`, Nextcloud preserves the aspect ratio (`a=1`).
    /// When `false`, Nextcloud crops or distorts (`a=0`).
    #[must_use]
    pub const fn keep_aspect_ratio(mut self, keep: bool) -> Self {
        self.keep_aspect_ratio = Some(keep);
        self
    }

    /// Sets the scaling mode (`mode` parameter).
    #[must_use]
    pub const fn mode(mut self, mode: PreviewMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// The desired preview width in pixels, if specified.
    pub const fn width_px(&self) -> Option<u32> {
        self.width
    }

    /// The desired preview height in pixels, if specified.
    pub const fn height_px(&self) -> Option<u32> {
        self.height
    }

    /// Whether aspect ratio should be preserved, if specified.
    pub const fn aspect_ratio(&self) -> Option<bool> {
        self.keep_aspect_ratio
    }

    /// The scaling mode, if specified.
    pub const fn preview_mode(&self) -> Option<PreviewMode> {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::PreviewOptions;
    use crate::mode::PreviewMode;

    #[test]
    fn default_options_are_empty() {
        let options = PreviewOptions::new();
        assert_eq!(options.width_px(), None);
        assert_eq!(options.height_px(), None);
        assert_eq!(options.aspect_ratio(), None);
        assert_eq!(options.preview_mode(), None);
    }

    #[test]
    fn builder_sets_all_options() {
        let options = PreviewOptions::new()
            .width(256)
            .height(512)
            .keep_aspect_ratio(true)
            .mode(PreviewMode::Cover);

        assert_eq!(options.width_px(), Some(256));
        assert_eq!(options.height_px(), Some(512));
        assert_eq!(options.aspect_ratio(), Some(true));
        assert_eq!(options.preview_mode(), Some(PreviewMode::Cover));
    }

    #[test]
    fn dimensions_helper_sets_both_axes() {
        let options = PreviewOptions::new().dimensions(1024, 768);
        assert_eq!(options.width_px(), Some(1024));
        assert_eq!(options.height_px(), Some(768));
    }
}
