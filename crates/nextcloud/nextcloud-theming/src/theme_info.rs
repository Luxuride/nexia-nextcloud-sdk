//! Server theming metadata models.

use serde::{Deserialize, Serialize};

/// Theming and branding configuration advertised by the Nextcloud server.
///
/// Returned by `GET /ocs/v2.php/apps/theming/api/v1/theme` or embedded within
/// server capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ThemeInfo {
    /// Instance product name or custom title (e.g. `"Nextcloud"` or `"ACME Cloud"`).
    #[serde(default)]
    pub name: Option<String>,
    /// Organization or product website URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Marketing slogan or login page tagline.
    #[serde(default)]
    pub slogan: Option<String>,
    /// Primary brand color in hexadecimal format (e.g. `"#0082c9"`).
    #[serde(default)]
    pub color: Option<String>,
    /// Accent color for interface elements.
    #[serde(default, alias = "color-element", alias = "color_element")]
    pub color_element: Option<String>,
    /// Element color adjusted for high contrast on bright backgrounds.
    #[serde(
        default,
        alias = "color-element-bright",
        alias = "color_element_bright"
    )]
    pub color_element_bright: Option<String>,
    /// Element color adjusted for high contrast on dark backgrounds.
    #[serde(default, alias = "color-element-dark", alias = "color_element_dark")]
    pub color_element_dark: Option<String>,
    /// Text color on top of the primary theme color.
    #[serde(default, alias = "color-text", alias = "color_text")]
    pub color_text: Option<String>,
    /// URL to the server's logo image.
    #[serde(default)]
    pub logo: Option<String>,
    /// URL to the server's header logo image.
    #[serde(default, alias = "logo-header", alias = "logo_header")]
    pub logo_header: Option<String>,
    /// URL to the instance favicon.
    #[serde(default)]
    pub favicon: Option<String>,
    /// URL to the instance login background image.
    #[serde(default)]
    pub background: Option<String>,
    /// Whether plain solid background coloring is enforced.
    #[serde(default, alias = "background-plain", alias = "background_plain")]
    pub background_plain: bool,
    /// Whether default server background is used.
    #[serde(default, alias = "background-default", alias = "background_default")]
    pub background_default: bool,
}

impl ThemeInfo {
    /// Instance title or branding name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Primary website link.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Marketing slogan or login tagline.
    pub fn slogan(&self) -> Option<&str> {
        self.slogan.as_deref()
    }

    /// Primary brand color in hex format (e.g. `"#0082c9"`).
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Element accent color in hex format.
    pub fn color_element(&self) -> Option<&str> {
        self.color_element.as_deref()
    }

    /// Text contrast color in hex format.
    pub fn color_text(&self) -> Option<&str> {
        self.color_text.as_deref()
    }

    /// URL to the brand logo.
    pub fn logo(&self) -> Option<&str> {
        self.logo.as_deref()
    }

    /// URL to the brand header logo.
    pub fn logo_header(&self) -> Option<&str> {
        self.logo_header.as_deref()
    }

    /// URL to the brand favicon.
    pub fn favicon(&self) -> Option<&str> {
        self.favicon.as_deref()
    }

    /// URL to the login background image.
    pub fn background(&self) -> Option<&str> {
        self.background.as_deref()
    }

    /// Whether a plain solid background is enabled.
    pub const fn is_background_plain(&self) -> bool {
        self.background_plain
    }

    /// Whether the default background image is in use.
    pub const fn is_background_default(&self) -> bool {
        self.background_default
    }
}
