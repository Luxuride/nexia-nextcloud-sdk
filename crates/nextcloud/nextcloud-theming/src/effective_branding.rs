//! Effective branding resolution result.

use serde::{Deserialize, Serialize};

/// Resolved effective branding combining client customization with server theme metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveBranding {
    /// Client application name.
    pub app_name: String,
    /// Remote server instance name (e.g. `"Nextcloud"` or custom organization title).
    pub instance_name: String,
    /// Marketing slogan or login tagline.
    pub slogan: Option<String>,
    /// Primary theme color in hex format (e.g. `"#0082c9"`).
    pub primary_color: String,
    /// Logo URL.
    pub logo_url: Option<String>,
    /// Website URL.
    pub website_url: Option<String>,
    /// Server background image URL.
    pub background_url: Option<String>,
}

impl EffectiveBranding {
    /// Client application name.
    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    /// Server instance title or brand name.
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Primary brand color in hex format.
    pub fn primary_color(&self) -> &str {
        &self.primary_color
    }

    /// Marketing slogan or tagline.
    pub fn slogan(&self) -> Option<&str> {
        self.slogan.as_deref()
    }

    /// Logo URL if available.
    pub fn logo_url(&self) -> Option<&str> {
        self.logo_url.as_deref()
    }

    /// Website URL if available.
    pub fn website_url(&self) -> Option<&str> {
        self.website_url.as_deref()
    }

    /// Background image URL if available.
    pub fn background_url(&self) -> Option<&str> {
        self.background_url.as_deref()
    }
}
