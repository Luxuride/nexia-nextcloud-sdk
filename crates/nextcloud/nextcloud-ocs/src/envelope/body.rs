use serde::{Deserialize, Serialize};

use super::Meta;

/// The `ocs` object of an OCS response: [`Meta`] plus the endpoint payload.
///
/// `data` is kept as the endpoint's own type so callers decode straight into
/// a domain struct instead of walking a generic JSON value.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Ocs<T> {
    meta: Meta,
    data: T,
}

impl<T> Ocs<T> {
    /// Verdict and pagination hints for this response.
    pub const fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Payload returned by the endpoint.
    pub const fn data(&self) -> &T {
        &self.data
    }

    /// Consumes the response, yielding the payload.
    pub fn into_data(self) -> T {
        self.data
    }
}
