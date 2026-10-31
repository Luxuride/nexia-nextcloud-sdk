//! Rust client for the Nextcloud APIs — the crate an application depends on.
//!
//! The workspace underneath this one owns the layers:
//! - `nexia-nextcloud-core` the HTTP vocabulary and the [`Transport`]/[`AuthProvider`] seams,
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
//! - `nextcloud-deck` Nextcloud Deck Kanban boards, stacks, and cards,
//! - `nextcloud-talk` Nextcloud Talk conversations and chat messaging,
//! - `nextcloud-preview` Nextcloud thumbnail and preview generation,
//! - `nextcloud-goa` GNOME Online Accounts desktop D-Bus single-sign-on integration,
//! - `nextcloud-kaccounts` KDE Plasma KAccounts desktop single-sign-on integration,
//! - `nextcloud-client-config` desktop client configuration parser and account discovery,
//! - `nextcloud-theming` theming, custom branding metadata, and unauthenticated server status,
//! - `nextcloud-search` Nextcloud Unified Search providers and queries,
//! - `nextcloud-sync` bidirectional synchronization engine, reconciliation, and sync journal,
//! - `nextcloud-sdk-vfs` transient mounting virtual tree with metadata and thumbnails,
//! - `nextcloud-sdk-fuse` transient VFS FUSE driver for desktop filesystem mounting,
//! - `nextcloud-sdk-orchestrator` background worker actor and multi-mount synchronization engine with sync status icons,
//! - `nextcloud-sdk-keyring` multi-platform keyring abstraction with Flatpak and native desktop support,
//! - `nextcloud-sdk-ratelimit` dynamic bandwidth rate limiting and transfer progress reporting,
//! - `nextcloud-sdk-storage` encrypted credential and cache database stores.
//!
//! This crate is the app-facing surface: it re-exports that vocabulary so one
//! dependency is enough, and adds the two state machines a client UI needs:
//!
//! - [`nexia_nextcloud_sdk_session`] — the account lifecycle, from "no server" to a
//!   usable session, following Nextcloud's documented Login Flow v2.
//! - [`nexia_nextcloud_sdk_request`] — the lifecycle of a single API call, including
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
pub mod event;
pub mod sso;
pub mod storage;
pub mod worker;

pub use builder::ClientBuilder;
pub use client::Client;
pub use error::ClientError;
pub use event::ClientEvent;
pub use nexia_nextcloud_activity::{Activity, ActivityClient, ActivityFilter};
pub use nexia_nextcloud_auth::{
    Anonymous, AppPassword, BasicAuth, BearerAuth, BearerToken, SecretString,
};
pub use nexia_nextcloud_client_config::{
    AuthType, ClientConfigError, ClientConfigParser, DesktopClientAccount, discover_config_paths,
    find_account_credentials, find_active_config_path,
};
pub use nexia_nextcloud_core::{
    ApiPath, AuthProvider, DecodeError, Error, Header, Headers, HttpMethod, Request, Response,
    ServerUrl, StatusCode, StatusError, Transport, UrlError,
};
pub use nexia_nextcloud_dav::{
    DavClient, DavPath, DavProperty, DavResource, Depth, Overwrite, PropfindBody,
};
pub use nexia_nextcloud_deck::{Board, BoardOwner, BoardPermissions, Card, DeckClient, Stack};
pub use nexia_nextcloud_goa::{GnomeOnlineAccounts, GoaAccount, GoaError};
pub use nexia_nextcloud_kaccounts::{
    KAccount, KAccountsDatabase, KAccountsError, discover_accounts_db_paths,
    find_active_accounts_db, find_kaccount_credentials,
};
pub use nexia_nextcloud_notifications::{Notification, NotificationAction, NotificationClient};
pub use nexia_nextcloud_ocs::{Meta, Ocs, OcsClient, OcsEnvelope, OcsFormat, OcsStatus, Query};
pub use nexia_nextcloud_preview::{
    PreviewClient, PreviewMode, PreviewOptions, crc32, embed_freedesktop_metadata,
    ensure_png_bytes, extract_embedded_jpeg_thumbnail, freedesktop_thumbnail_dirs,
    freedesktop_thumbnail_is_current, install_freedesktop_fail_marker,
    install_freedesktop_thumbnail, install_freedesktop_thumbnail_with_mtime, make_png_text_chunk,
    md5_hex, path_to_file_uri, remove_freedesktop_fail_marker,
};
pub use nexia_nextcloud_sdk_fuse::{
    AsyncHydrator, BackgroundSession, DirEntry, FileAttr, FileType, FuseDriver, InodeTable,
    MountOption, mount_virtual_fs, parse_date_to_unix_secs,
};
pub use nexia_nextcloud_sdk_keyring::{
    KeyringBackend, KeyringEntry, KeyringError, MemoryKeyring, PlatformKeyring,
};
pub use nexia_nextcloud_sdk_orchestrator::{
    DownloadQueueCoordinator, FileSystemWatcher, FolderSyncOutcome, FolderSyncRunner,
    HydrationCallback, MountDescriptor, MountId, MountMode, MountProgressSnapshot, MountState,
    MountStatus, MountStorageUsage, MountsHandle, NextcloudOrchestrator, OrchestratorConfig,
    OrchestratorError, OrchestratorEvent, OrchestratorHandle, OrchestratorResult, SyncCallback,
    SyncFileStatus, VfsHandle, VfsManager, make_dav_path,
};
pub use nexia_nextcloud_sdk_request::{Effect, Failure, RequestMachine, RequestState, RetryPolicy};
pub use nexia_nextcloud_sdk_session::{
    Account, Command, LoginFlow, LoginStart, LoginSuccess, PollTarget, PollToken, SessionError,
    SessionEvent, SessionMachine, SessionState,
};
pub use nexia_nextcloud_sdk_storage::{
    EncryptedDatabase, KeyStore, KeychainKeyStore, StorageError, StoragePaths, StoreKey,
};
pub use nexia_nextcloud_sdk_vfs::{
    BreadcrumbSegment, DavResourceSummary, DirectorySizeSummary, FileSortCriteria, FileSortField,
    FileSortOrder, HierarchySeeder, HydratedContent, HydrationState, NavigationHistory,
    NextcloudPermissions, NodeMetadata, ResourceIconCategory, StandardNextcloudSeeder,
    ThumbnailCache, VfsError, VirtualEntrySummary, VirtualNode, VirtualTree, breadcrumb_segments,
    file_name, format_bytes, normalize_path, parent_path,
};
pub use nexia_nextcloud_search::{SearchClient, SearchEntry, SearchProvider, SearchResultSet};
pub use nexia_nextcloud_shares::{CreateShare, Share, ShareClient, SharePermissions, ShareType};
pub use nexia_nextcloud_sync::{
    DiscoveredLocalFile, DiscoveredRemoteFile, DownloadExecutor, DownloadPriority,
    DownloadProgress, DownloadQueueEvent, DownloadQueueSnapshot, DownloadStatus, DownloadTask,
    DownloadTaskId, JournalEntry, SyncAction, SyncDownloadQueue, SyncError, SyncJournal, reconcile,
    scan_local_directory,
};
pub use nexia_nextcloud_talk::{ChatMessage, Room, RoomType, TalkClient};
pub use nexia_nextcloud_theming::{
    CustomBranding, EffectiveBranding, ServerStatusInfo, ThemeInfo, ThemingClient, ThemingError,
};
pub use nexia_nextcloud_transport::{
    BandwidthLimit, BandwidthLimiter, ProgressCallback, ReqwestTransport, TimeoutConfig,
    TransferProgress, TransportBuildError, TransportBuilder,
};
pub use nexia_nextcloud_trashbin::{TrashClient, TrashItem};
pub use nexia_nextcloud_uploads::{ChunkBoundary, UploadClient, UploadSession};
pub use nexia_nextcloud_users::{
    Capabilities, ChunkedUpload, CoreCapabilities, FilesCapabilities, Quota, ServerStatus,
    ServerVersion, ThemingCapabilities, User, UserClient,
};
pub use nexia_nextcloud_versions::{FileVersion, VersionsClient};
pub use sso::{AccountSource, DesktopSso, DiscoveredAccount, SsoError};
pub use storage::{
    CacheStore, CachedDavEntry, CachedTheme, ConfigStore, SessionStore, StoredSyncMount,
};
pub use worker::{WorkerActor, WorkerStores, WorkerTask};
