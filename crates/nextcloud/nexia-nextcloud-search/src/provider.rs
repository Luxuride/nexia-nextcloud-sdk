//! Domain model representing a Nextcloud unified search provider.

use serde::{Deserialize, Serialize};

/// A registered search provider in Nextcloud (e.g. files, apps, bookmarks).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchProvider {
    /// Unique provider ID (e.g. "files").
    pub id: String,
    /// Human-readable display name of the provider.
    pub name: String,
    /// Optional CSS icon class or icon identifier.
    #[serde(default)]
    pub icon: Option<String>,
    /// Order weighting among search providers.
    #[serde(default)]
    pub order: i32,
}
