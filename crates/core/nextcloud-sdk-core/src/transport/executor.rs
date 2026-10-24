use std::future::Future;

use crate::error::Error;

use super::{Request, Response};

/// Carries requests to a Nextcloud instance.
///
/// Implementations are stateless with respect to a single call and must be
/// shareable, so a client can clone references into spawned tasks. The
/// returned future is `Send + 'static` (the request is consumed, `&self`
/// lives only for the call) which keeps callers free to `tokio::spawn`.
pub trait Transport: Send + Sync {
    /// Executes `request`, returning the raw response.
    ///
    /// A non-2xx response is *not* an error here: API areas decide what a
    /// given status means for their endpoints, and the OCS envelope often
    /// carries a meaningful body alongside a failing status.
    fn execute(&self, request: Request) -> impl Future<Output = Result<Response, Error>> + Send;
}

impl<T: Transport + ?Sized> Transport for &T {
    fn execute(&self, request: Request) -> impl Future<Output = Result<Response, Error>> + Send {
        (**self).execute(request)
    }
}
