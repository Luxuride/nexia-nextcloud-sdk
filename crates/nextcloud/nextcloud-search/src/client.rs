//! Nextcloud Unified Search API client.

use nextcloud_ocs::{OcsClient, OcsFormat, Query};
use nextcloud_sdk_core::{ApiPath, AuthProvider, Error, Transport};

use crate::provider::SearchProvider;
use crate::result::SearchResultSet;

#[cfg(test)]
mod tests;

/// Relative path to Nextcloud's Unified Search API beneath `/ocs/v2.php`.
pub const SEARCH_API_PATH: &str = "search/providers";

/// Client for interacting with Nextcloud's Unified Search API.
///
/// Wraps an [`OcsClient`] by reference and provides high-level typed calls for:
/// - Discovering registered search providers (`/ocs/v2.php/search/providers`)
/// - Querying matches from a specific search provider (`/ocs/v2.php/search/providers/{provider_id}/search`)
#[derive(Clone, Copy, Debug)]
pub struct SearchClient<'a, T: Transport, A: AuthProvider> {
    ocs: &'a OcsClient<T, A>,
}

impl<'a, T: Transport, A: AuthProvider> SearchClient<'a, T, A> {
    /// Binds a search client to an underlying [`OcsClient`].
    pub const fn new(ocs: &'a OcsClient<T, A>) -> Self {
        Self { ocs }
    }

    /// Borrows the underlying [`OcsClient`].
    pub const fn ocs(&self) -> &'a OcsClient<T, A> {
        self.ocs
    }

    /// Lists all available unified search providers.
    ///
    /// Sends `GET /ocs/v2.php/search/providers?format=json`.
    ///
    /// # Errors
    /// Returns [`Error::Status`] on non-2xx responses or [`Error::Transport`] on network errors.
    pub async fn list_providers(&self) -> Result<Vec<SearchProvider>, Error> {
        tracing::debug!("Listing unified search providers");
        let path = ApiPath::ocs(SEARCH_API_PATH)?;
        self.ocs
            .get(path, Query::new().format(OcsFormat::Json))
            .await
    }

    /// Searches across a specific provider (such as `"files"`).
    ///
    /// Sends `GET /ocs/v2.php/search/providers/{provider_id}/search?term={term}&format=json`.
    ///
    /// # Errors
    /// Returns [`Error::Status`] on non-2xx responses or [`Error::Transport`] on network errors.
    pub async fn search(&self, provider_id: &str, term: &str) -> Result<SearchResultSet, Error> {
        self.search_with_options(provider_id, term, None, None)
            .await
    }

    /// Searches across a specific provider with optional limit and cursor pagination.
    ///
    /// Sends `GET /ocs/v2.php/search/providers/{provider_id}/search?term={term}&limit={limit}&cursor={cursor}&format=json`.
    ///
    /// # Errors
    /// Returns [`Error::Status`] on non-2xx responses or [`Error::Transport`] on network errors.
    pub async fn search_with_options(
        &self,
        provider_id: &str,
        term: &str,
        limit: Option<u32>,
        cursor: Option<i64>,
    ) -> Result<SearchResultSet, Error> {
        tracing::debug!(provider_id, term, "Executing unified search query");
        let path = ApiPath::ocs(format!("{SEARCH_API_PATH}/{provider_id}/search"))?;
        let mut query = Query::new().format(OcsFormat::Json).param("term", term);
        if let Some(l) = limit {
            query = query.param("limit", l.to_string());
        }
        if let Some(c) = cursor {
            query = query.param("cursor", c.to_string());
        }
        self.ocs.get(path, query).await
    }
}
