//! Nextcloud Unified Search API client and domain models.
//!
//! Nextcloud provides an endpoint for querying unified search providers (files, apps, bookmarks, etc.)
//! at `/ocs/v2.php/search/providers`.
//!
//! - [`SearchClient`] — asynchronous client for querying search providers and results.
//! - [`SearchProvider`] — registered search provider metadata.
//! - [`SearchResultSet`] — matching results from a provider query.
//! - [`SearchEntry`] — individual search result match with title, path/subline, and preview URL.
//!
//! Official Nextcloud Search documentation:
//! <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html>
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod provider;
mod result;

pub use client::{SEARCH_API_PATH, SearchClient};
pub use provider::SearchProvider;
pub use result::{SearchEntry, SearchResultSet};
