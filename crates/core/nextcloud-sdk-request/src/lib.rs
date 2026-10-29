//! The lifecycle of one API call: sending, retrying, finishing.
//!
//! The machine answers two questions for an application: *is this call still
//! worth retrying*, and *how far along is it*. Both are pure decisions —
//! [`RequestMachine`] never sleeps and never opens a connection; it returns an
//! [`Effect`] the caller loop performs.
//!
//! Retry eligibility mirrors [`nexia_nextcloud_core::Error::is_retryable`]: transport
//! failures, `5xx` and `429` are retried, everything else is final. Delays grow
//! exponentially from a configurable base and are capped, so a long outage
//! cannot push the next attempt far into the future.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod failure;
mod machine;
mod policy;
mod state;

pub use failure::Failure;
pub use machine::{Effect, RequestMachine};
pub use policy::RetryPolicy;
pub use state::RequestState;
