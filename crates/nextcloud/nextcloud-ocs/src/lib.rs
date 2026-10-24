//! OCS API envelope and client for the Nextcloud SDK.
//!
//! Nextcloud's OCS endpoints answer with an envelope that wraps the payload in
//! a status block, so decoding an OCS response is two steps: unwrap the
//! envelope, then read the `data` field.
//!
//! - [`OcsEnvelope`] / [`Ocs`] / [`Meta`] — the wire shape of that envelope,
//!   tolerant of the field types Nextcloud actually sends.
//! - [`OcsStatus`] — the verdict carried by `meta.statuscode`.
//! - [`Query`] / [`OcsFormat`] — the query string appended to an OCS path,
//!   including the `format=json` selector.
//! - [`OcsClient`] — executes an OCS call through any
//!   [`Transport`](nextcloud_sdk_core::Transport) and returns the decoded payload.
//!
//! OCS requires two request headers that are easy to forget: every call must
//! send `OCS-APIRequest: true`, and JSON answers require
//! `Accept: application/json`. [`OcsClient`] sets both.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod envelope;
mod error;
mod query;

pub use client::OcsClient;
pub use envelope::{Meta, Ocs, OcsEnvelope, OcsStatus};
pub use query::{OcsFormat, Query};
