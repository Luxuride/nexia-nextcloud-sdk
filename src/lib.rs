//! Rust client for the Nextcloud APIs — the crate an application depends on.
//!
//! The workspace underneath this one owns the layers:
//! - `sdk-core` the HTTP vocabulary and the [`Transport`]/[`AuthProvider`] seams,
//! - `nextcloud-auth` the credential types,
//! - `nextcloud-transport` the `reqwest` implementation,
//! - `nextcloud-ocs` the OCS envelope and mutation engine,
//! - `nextcloud-dav` the WebDAV file and directory CRUD operations,
//! - `nextcloud-users` server capabilities and user profiles,
//! - `nextcloud-shares` OCS file sharing and public links,
//! - `nextcloud-notifications` Nextcloud user notifications,
//! - `nextcloud-activity` activity feed streams,
//! - `nextcloud-trashbin` WebDAV trashbin management,
//! - `nextcloud-versions` WebDAV file versioning history and restoration,
//! - `nextcloud-uploads` WebDAV chunked upload streaming v2,
//! - `nextcloud-preview` Nextcloud thumbnail and preview generation,
//! - `nextcloud-goa` GNOME Online Accounts desktop D-Bus single-sign-on integration,
//! - `sdk-vfs` transient mounting virtual tree with metadata and thumbnails,
//! - `sdk-storage` encrypted credential and cache database stores.
//!
//! This crate is the app-facing surface: it re-exports that vocabulary so one
//! dependency is enough, and adds the two state machines a client UI needs:
//!
//! - [`sdk_session`] — the account lifecycle, from "no server" to a
//!   usable session, following Nextcloud's documented Login Flow v2.
//! - [`sdk_request`] — the lifecycle of a single API call, including
//!   whether to retry and what to show as progress.
//!
//! Both machines are pure transition tables: they perform no I/O, and return
//! the work still to be done ([`Command`], [`Effect`]) so an
//! application can drive them from any runtime and test them without a
//! server.
//!
//! The top-level [`Client`] brings everything together into a stateful,
//! actor-driven handle configurable via [`ClientBuilder`].
#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod builder;
pub mod client;
pub mod error;
pub mod storage;
pub mod worker;

pub use builder::ClientBuilder;
pub use client::Client;
pub use error::ClientError;
pub use nextcloud_activity::{Activity, ActivityClient, ActivityFilter};
pub use nextcloud_auth::{
    Anonymous, AppPassword, BasicAuth, BearerAuth, BearerToken, SecretString,
};
pub use nextcloud_dav::{
    DavClient, DavPath, DavProperty, DavResource, Depth, Overwrite, PropfindBody,
};
pub use nextcloud_goa::{GnomeOnlineAccounts, GoaAccount, GoaError};
pub use nextcloud_notifications::{Notification, NotificationAction, NotificationClient};
pub use nextcloud_ocs::{Meta, Ocs, OcsClient, OcsEnvelope, OcsFormat, OcsStatus, Query};
pub use nextcloud_preview::{PreviewClient, PreviewMode, PreviewOptions};
pub use nextcloud_shares::{CreateShare, Share, ShareClient, SharePermissions, ShareType};
pub use nextcloud_transport::{
    ReqwestTransport, TimeoutConfig, TransportBuildError, TransportBuilder,
};
pub use nextcloud_trashbin::{TrashClient, TrashItem};
pub use nextcloud_uploads::{ChunkBoundary, UploadClient, UploadSession};
pub use nextcloud_users::{
    Capabilities, ChunkedUpload, CoreCapabilities, FilesCapabilities, Quota, ServerStatus,
    ServerVersion, ThemingCapabilities, User, UserClient,
};
pub use nextcloud_versions::{FileVersion, VersionsClient};
pub use sdk_core::{
    ApiPath, AuthProvider, DecodeError, Error, Header, Headers, HttpMethod, Request, Response,
    ServerUrl, StatusCode, StatusError, Transport, UrlError,
};
pub use sdk_request::{Effect, Failure, RequestMachine, RequestState, RetryPolicy};
pub use sdk_session::{
    Account, Command, LoginFlow, LoginStart, LoginSuccess, PollTarget, PollToken, SessionError,
    SessionEvent, SessionMachine, SessionState,
};
pub use sdk_storage::{
    EncryptedDatabase, KeyStore, KeychainKeyStore, StorageError, StoragePaths, StoreKey,
};
pub use sdk_vfs::{
    HydratedContent, HydrationState, NodeMetadata, ThumbnailCache, VfsError, VirtualNode,
    VirtualTree,
};
pub use storage::{CacheStore, CachedDavEntry, SessionStore};
pub use worker::{WorkerActor, WorkerTask};
