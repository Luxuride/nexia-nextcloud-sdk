use serde::{Deserialize, Serialize};

use crate::wire::lossy_bool;

/// Theming and branding customization capabilities exposed by the server.
///
/// Nextcloud instances with the `theming` app enabled return visual styling
/// parameters including instance title, primary brand color, and asset URLs.
///
/// See the [Nextcloud Theming Documentation](https://docs.nextcloud.com/server/latest/admin_manual/configuration_server/theming.html).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ThemingCapabilities {
    /// Instance product name or custom server name.
    #[serde(default)]
    pub name: Option<String>,
    /// Organization or website URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Slogan or tagline displayed on the login page.
    #[serde(default)]
    pub slogan: Option<String>,
    /// Primary theme color in hexadecimal format (e.g. `"#0082c9"`).
    #[serde(default)]
    pub color: Option<String>,
    /// Secondary or element highlight color in hex format.
    #[serde(default, alias = "color-element", alias = "color_element")]
    pub color_element: Option<String>,
    /// Element color adjusted for bright backgrounds.
    #[serde(
        default,
        alias = "color-element-bright",
        alias = "color_element_bright"
    )]
    pub color_element_bright: Option<String>,
    /// Element color adjusted for dark backgrounds.
    #[serde(default, alias = "color-element-dark", alias = "color_element_dark")]
    pub color_element_dark: Option<String>,
    /// Header or button text contrast color in hex format.
    #[serde(default, alias = "color-text", alias = "color_text")]
    pub color_text: Option<String>,
    /// URL to the server's custom logo image.
    #[serde(default)]
    pub logo: Option<String>,
    /// URL to the server's login background image.
    #[serde(default)]
    pub background: Option<String>,
    /// Whether plain background coloring is enforced.
    #[serde(
        default,
        alias = "background-plain",
        alias = "background_plain",
        deserialize_with = "lossy_bool"
    )]
    pub background_plain: bool,
    /// Whether default server background is used.
    #[serde(
        default,
        alias = "background-default",
        alias = "background_default",
        deserialize_with = "lossy_bool"
    )]
    pub background_default: bool,
}

impl ThemingCapabilities {
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

    /// Primary brand color.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Element accent color.
    pub fn color_element(&self) -> Option<&str> {
        self.color_element.as_deref()
    }

    /// Element accent color on bright backgrounds.
    pub fn color_element_bright(&self) -> Option<&str> {
        self.color_element_bright.as_deref()
    }

    /// Element accent color on dark backgrounds.
    pub fn color_element_dark(&self) -> Option<&str> {
        self.color_element_dark.as_deref()
    }

    /// Contrast text color.
    pub fn color_text(&self) -> Option<&str> {
        self.color_text.as_deref()
    }

    /// URL to the instance logo.
    pub fn logo(&self) -> Option<&str> {
        self.logo.as_deref()
    }

    /// URL to the instance background.
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
