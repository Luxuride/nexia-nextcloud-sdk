//! Credential types for the Nextcloud SDK.
//!
//! Every provider here implements [`nexia_nextcloud_core::AuthProvider`] and only
//! mutates request headers, so callers stay generic over the credential
//! format:
//!
//! - [`BasicAuth`] — user name plus app password, `Authorization: Basic`.
//! - [`BearerAuth`] — OAuth2 access token, `Authorization: Bearer`.
//! - [`Anonymous`] — no credentials, plus the `OCS-APIRequest` marker OCS
//!   endpoints require.
//!
//! Secrets are held in [`SecretString`], whose `Debug` output is redacted and
//! which deliberately has neither a `Display` nor a `Serialize` impl.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod anonymous;
mod basic;
mod bearer;
mod secret;

pub use anonymous::Anonymous;
pub use basic::{AppPassword, BasicAuth};
pub use bearer::{BearerAuth, BearerToken};
pub use secret::SecretString;
