//! Client-side custom branding configuration and builder.

use serde::{Deserialize, Serialize};

use crate::effective_branding::EffectiveBranding;
use crate::server_status::ServerStatusInfo;
use crate::theme_info::ThemeInfo;

/// Client application custom branding metadata.
///
/// Allows client distributions (such as desktop apps, CLI utilities, or white-labeled
/// Nextcloud builds) to provide custom branding, titles, colors, and user agent parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomBranding {
    /// Client application name (e.g. `"ACME Sync"` or `"Example Files"`).
    pub app_name: String,
    /// Client application version (e.g. `"1.2.0"`).
    pub app_version: Option<String>,
    /// Organization or vendor name.
    pub vendor: Option<String>,
    /// Primary brand color in hex format (e.g. `"#0082c9"`).
    pub primary_color: Option<String>,
    /// Custom logo URL or resource identifier.
    pub logo_url: Option<String>,
    /// Support or organization website URL.
    pub website_url: Option<String>,
    /// Tagline or slogan.
    pub slogan: Option<String>,
    /// Explicit User-Agent override string.
    pub user_agent: Option<String>,
}

impl CustomBranding {
    /// Creates a new `CustomBranding` with the given application name.
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            app_version: None,
            vendor: None,
            primary_color: None,
            logo_url: None,
            website_url: None,
            slogan: None,
            user_agent: None,
        }
    }

    /// Sets the application version.
    #[must_use]
    pub fn with_app_version(mut self, version: impl Into<String>) -> Self {
        self.app_version = Some(version.into());
        self
    }

    /// Sets the vendor or organization name.
    #[must_use]
    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }

    /// Sets the primary brand color in hex format (e.g. `"#0082c9"`).
    #[must_use]
    pub fn with_primary_color(mut self, color: impl Into<String>) -> Self {
        self.primary_color = Some(color.into());
        self
    }

    /// Sets the brand logo URL.
    #[must_use]
    pub fn with_logo_url(mut self, url: impl Into<String>) -> Self {
        self.logo_url = Some(url.into());
        self
    }

    /// Sets the organization website URL.
    #[must_use]
    pub fn with_website_url(mut self, url: impl Into<String>) -> Self {
        self.website_url = Some(url.into());
        self
    }

    /// Sets the brand slogan or tagline.
    #[must_use]
    pub fn with_slogan(mut self, slogan: impl Into<String>) -> Self {
        self.slogan = Some(slogan.into());
        self
    }

    /// Sets an explicit HTTP User-Agent header value.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Generates the standard HTTP User-Agent string for this branding.
    ///
    /// If an explicit user agent was set with [`Self::with_user_agent`], returns it directly;
    /// otherwise constructs `"AppName/Version (Vendor)"` or `"AppName"`.
    pub fn build_user_agent(&self) -> String {
        if let Some(ref ua) = self.user_agent {
            return ua.clone();
        }

        let mut out = self.app_name.clone();
        if let Some(ref ver) = self.app_version {
            out.push('/');
            out.push_str(ver);
        }
        if let Some(ref vendor) = self.vendor {
            out.push_str(" (");
            out.push_str(vendor);
            out.push(')');
        }
        out
    }

    /// Resolves effective branding by merging client overrides with server theme info and status.
    pub fn resolve(
        &self,
        server_theme: Option<&ThemeInfo>,
        server_status: Option<&ServerStatusInfo>,
    ) -> EffectiveBranding {
        let instance_name = server_theme
            .and_then(|t| t.name.clone())
            .or_else(|| server_status.and_then(|s| s.product_name.clone()))
            .unwrap_or_else(|| "Nextcloud".to_string());

        let slogan = self
            .slogan
            .clone()
            .or_else(|| server_theme.and_then(|t| t.slogan.clone()));

        let primary_color = self
            .primary_color
            .clone()
            .or_else(|| server_theme.and_then(|t| t.color.clone()))
            .unwrap_or_else(|| "#0082c9".to_string());

        let logo_url = self
            .logo_url
            .clone()
            .or_else(|| server_theme.and_then(|t| t.logo.clone()));

        let website_url = self
            .website_url
            .clone()
            .or_else(|| server_theme.and_then(|t| t.url.clone()));

        let background_url = server_theme.and_then(|t| t.background.clone());

        EffectiveBranding {
            app_name: self.app_name.clone(),
            instance_name,
            slogan,
            primary_color,
            logo_url,
            website_url,
            background_url,
        }
    }
}
