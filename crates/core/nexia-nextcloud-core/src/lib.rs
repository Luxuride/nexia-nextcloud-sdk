//! Core primitives shared by the Nextcloud SDK crates.
//!
//! This crate owns the vocabulary the other crates speak: HTTP methods,
//! status codes and headers ([`http`]), Nextcloud base URLs and request
//! paths ([`url`]), the [`Transport`] abstraction every request goes
//! through ([`transport`]), credential application ([`auth`]) and the
//! generic [`Error`] type.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod auth;
pub mod error;
pub mod http;
pub mod transport;
pub mod url;

pub use auth::AuthProvider;
pub use error::{DecodeError, Error, MAX_BODY_LEN, StatusError};
pub use http::{Header, Headers, HttpMethod, StatusCode};
pub use transport::{Request, Response, Transport};
pub use url::{ApiPath, ServerUrl, UrlError};
