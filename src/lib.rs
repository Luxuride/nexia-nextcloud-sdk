//! Rust client for the Nextcloud APIs — the crate an application depends on.
//!
//! The workspace underneath this one owns the layers: `sdk-core` the
//! HTTP vocabulary and the [`Transport`]/[`AuthProvider`] seams,
//! `nextcloud-auth` the credential types, `nextcloud-transport` the `reqwest`
//! implementation, `nextcloud-ocs` the OCS envelope and `nextcloud-dav` the
//! WebDAV foundation. This crate is the app-facing surface: it re-exports that
//! vocabulary so one dependency is enough, and adds the two state machines a
//! client UI needs.
//!
//! - [`sdk_session`] — the account lifecycle, from "no server" to a
//!   usable session, following Nextcloud's documented Login Flow v2.
//! - [`sdk_request`] — the lifecycle of a single API call, including
//!   whether to retry and what to show as progress.
//!
//! Both machines are pure transition tables: they perform no I/O, and return
//! the work still to be done ([`session::Command`], [`request::Effect`]) so an
//! application can drive them from any runtime and test them without a
//! server.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub use nextcloud_auth::{
    Anonymous, AppPassword, BasicAuth, BearerAuth, BearerToken, SecretString,
};
pub use nextcloud_dav::{DavClient, DavPath, DavProperty, DavResource, Depth, PropfindBody};
pub use nextcloud_ocs::{Meta, Ocs, OcsClient, OcsEnvelope, OcsFormat, OcsStatus, Query};
pub use nextcloud_transport::{
    ReqwestTransport, TimeoutConfig, TransportBuildError, TransportBuilder,
};
pub use sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Header, Headers, HttpMethod, Request, Response,
    ServerUrl, StatusCode, StatusError, Transport, UrlError,
};
pub use sdk_request::{Effect, Failure, RequestMachine, RequestState, RetryPolicy};
pub use sdk_session::{
    Account, Command, LoginFlow, LoginStart, LoginSuccess, PollTarget, PollToken, SessionError,
    SessionEvent, SessionMachine, SessionState,
};
