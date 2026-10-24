use serde::Deserialize;

use super::Meta;

/// The parts of an envelope needed to read its verdict, without the payload.
///
/// The verdict has to be read before the payload is decoded: OCS failures
/// carry whatever shape the endpoint felt like (`[]`, `{}`, or an error
/// object), so decoding `data` first would report a well-explained
/// `meta.statuscode` as an unreadable body and lose the OCS code.
#[derive(Debug, Deserialize)]
pub(crate) struct VerdictEnvelope {
    pub(crate) ocs: VerdictBody,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VerdictBody {
    pub(crate) meta: Meta,
}
