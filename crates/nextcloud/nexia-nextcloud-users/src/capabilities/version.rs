use serde::{Deserialize, Serialize};

/// Nextcloud server version information reported in capabilities.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServerVersion {
    /// Major release version.
    pub major: u32,
    /// Minor release version, if present.
    #[serde(default)]
    pub minor: Option<u32>,
    /// Micro / patch release version, if present.
    #[serde(default)]
    pub micro: Option<u32>,
    /// Full version string (e.g. `"30.0.4"`).
    pub string: String,
    /// Nextcloud edition (e.g. `"Community"` or `"Enterprise"`), if reported.
    #[serde(default)]
    pub edition: Option<String>,
    /// Whether this server has extended support enabled.
    #[serde(default, alias = "extendedSupport", alias = "extended_support")]
    pub extended_support: Option<bool>,
}

impl ServerVersion {
    /// Major release version number.
    pub const fn major(&self) -> u32 {
        self.major
    }

    /// Minor release version number.
    pub const fn minor(&self) -> Option<u32> {
        self.minor
    }

    /// Micro / patch release version number.
    pub const fn micro(&self) -> Option<u32> {
        self.micro
    }

    /// Full version string.
    pub fn string(&self) -> &str {
        &self.string
    }

    /// Server edition name.
    pub fn edition(&self) -> Option<&str> {
        self.edition.as_deref()
    }

    /// Whether extended support is active.
    pub const fn extended_support(&self) -> Option<bool> {
        self.extended_support
    }
}
