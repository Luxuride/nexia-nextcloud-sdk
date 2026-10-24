use serde::{Deserialize, Serialize};

use super::{Meta, Ocs};

/// Top level of every JSON OCS response: `{"ocs": {...}}`.
///
/// The wrapper exists because OCS also answers in XML, where the same shape
/// appears as an `ocs` element; the name is part of the wire format, not a
/// decoration. A response without it is a decode failure, not an empty
/// payload.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OcsEnvelope<T> {
    ocs: Ocs<T>,
}

impl<T> OcsEnvelope<T> {
    /// The wrapped [`Ocs`] block.
    pub const fn ocs(&self) -> &Ocs<T> {
        &self.ocs
    }

    /// Verdict and pagination hints for this response.
    pub const fn meta(&self) -> &Meta {
        self.ocs.meta()
    }

    /// Payload returned by the endpoint.
    pub const fn data(&self) -> &T {
        self.ocs.data()
    }

    /// Consumes the envelope, yielding the payload.
    pub fn into_data(self) -> T {
        self.ocs.into_data()
    }
}
