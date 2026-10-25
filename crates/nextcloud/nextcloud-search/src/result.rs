//! Domain models representing Nextcloud search results and individual search entries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An individual search result entry returned by a search provider.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchEntry {
    /// Primary title of the search match (e.g. file name or headline).
    pub title: String,
    /// Subtitle or secondary detail path (e.g. directory path "/Documents/foo").
    #[serde(default)]
    pub subline: Option<String>,
    /// Resource URL on Nextcloud (e.g. direct link to open the file).
    #[serde(default, rename = "resourceUrl")]
    pub resource_url: Option<String>,
    /// URL to fetch a thumbnail preview of the item, if supported.
    #[serde(default, rename = "thumbnailUrl")]
    pub thumbnail_url: Option<String>,
    /// Icon identifier or mime-type icon class.
    #[serde(default)]
    pub icon: Option<String>,
    /// Whether the avatar/icon should be styled rounded.
    #[serde(default)]
    pub rounded: bool,
    /// Extra attributes attached to the search match.
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// Paginated or unpaginated result set from querying a search provider.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SearchResultSet {
    /// Human-readable name of the searching provider.
    pub name: String,
    /// Whether additional result pages are available via cursor.
    #[serde(default, rename = "isPaginated")]
    pub is_paginated: bool,
    /// Matching entries found by the search provider.
    #[serde(default)]
    pub entries: Vec<SearchEntry>,
    /// Cursor token for fetching the next page of results, if paginated.
    #[serde(default)]
    pub cursor: Option<i64>,
}
