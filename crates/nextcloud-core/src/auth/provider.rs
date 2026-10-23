use crate::http::Headers;

/// Supplies credentials to outgoing requests.
///
/// Only the header mutation is part of the contract, not the credential
/// format: app passwords, bearer tokens and anonymous access all implement
/// this trait in `nextcloud-auth`, and callers stay generic over it.
pub trait AuthProvider: Send + Sync {
    /// Applies this provider's credentials to `headers`.
    ///
    /// Implementations replace any existing `Authorization` header so a
    /// retry with refreshed credentials cannot leave two of them behind.
    fn authenticate(&self, headers: &mut Headers);
}
