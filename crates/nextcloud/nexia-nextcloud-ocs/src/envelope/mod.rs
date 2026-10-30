//! The OCS response envelope: a `meta` verdict plus a `data` payload.

mod body;
mod meta;
mod ocs_envelope;
mod status;
mod text;
mod verdict;

pub use body::Ocs;
pub use meta::Meta;
pub use ocs_envelope::OcsEnvelope;
pub use status::OcsStatus;

pub(crate) use verdict::VerdictEnvelope;
