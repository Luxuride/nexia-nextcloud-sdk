# Nextcloud Rust SDK — API Reference & Endpoint Matrix

A mapping between Nextcloud server HTTP endpoints, the implementing Rust crates,
their typed client methods, and the structures they return.

Every client in this table is **stateless**: it borrows a `Transport` and an
`AuthProvider`, performs one operation and returns owned data. None of them keep
a mount, an ETag cache or a retry timer — that state lives in
`nextcloud-sdk-orchestrator`. See
[`ARCHITECTURE.md`](ARCHITECTURE.md#1-layers) for the layering rule.

Reference implementation: `nextcloud-rust-sdk` version `0.1.3`, workspace
`members = ["crates/*/*"]`.

---

## 1. Official Nextcloud developer documentation

All protocol crates are written against the official specifications:

- [Nextcloud Developer Manual (GitHub master)](https://github.com/nextcloud/documentation/tree/master/developer_manual)
- [Login Flow v2 Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/LoginFlow/index.rst)
- [OCS API Overview & Protocol Guidelines](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-api-overview.rst)
- [WebDAV Endpoints & Multi-Status Protocol](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/index.rst)
- [OCS Share API Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst)
- [Notifications API v2 Specification](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md)
- [Activity API v2 Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst)
- [User Provisioning API](https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html)

---

## 2. Endpoint matrix

### Authentication & login

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/index.php/login/v2` | `POST` | `nextcloud-sdk-session` | `SessionMachine::handle(SessionEvent::StartRequested)` → `Command::StartLogin` | — | `LoginFlow` |
| `/login/v2/poll` | `POST` | `nextcloud-sdk-session` | `SessionMachine::handle(SessionEvent::PollPending)` → `Command::Poll(PollTarget)` | `PollTarget { endpoint, token }` | `SessionState` |

At the facade level these are `Client::start_login() -> LoginStart` and
`Client::poll_login() -> SessionState`; the worker owns the HTTP call, the state
machine only decides what to do next.

### Server capabilities & users

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/cloud/capabilities` | `GET` | `nextcloud-users` | `UserClient::capabilities()` | — (`format=json`) | `Capabilities` |
| `/ocs/v2.php/cloud/user` | `GET` | `nextcloud-users` | `UserClient::current_user()` | — | `User` |
| `/ocs/v2.php/cloud/users/{id}` | `GET` | `nextcloud-users` | `UserClient::get_user(user_id)` | `user_id: &str` | `User` |
| `/status.php` | `GET` | `nextcloud-theming` | `ThemingClient::server_status()` | — (unauthenticated) | `ServerStatusInfo` |

`Capabilities` accessors: `server_version()`, `version()`, `status()`, `theming()`,
`core()` → `webdav_root()`, `files()` → `chunked_upload()`, `max_chunk_size()`.

### WebDAV file storage

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/remote.php/dav/files/{user}/{path}` | `PROPFIND` | `nextcloud-dav` | `DavClient::propfind(path, depth, body)` | `&DavPath`, `Depth`, `&PropfindBody` | `Vec<DavResource>` |
| `/remote.php/dav/files/{user}/{path}` | `GET` | `nextcloud-dav` | `DavClient::get(path)` | `&DavPath` | `Vec<u8>` |
| `/remote.php/dav/files/{user}/{path}` | `GET` (+`Range`) | `nextcloud-dav` | `DavClient::get_range(path, start, end)` | `&DavPath`, `u64`, `u64` | `Vec<u8>` |
| `/remote.php/dav/files/{user}/{path}` | `PUT` | `nextcloud-dav` | `DavClient::put(path, body, content_type)` | `&DavPath`, `Vec<u8>`, `Option<&str>` | `()` |
| `/remote.php/dav/files/{user}/{path}` | `MKCOL` | `nextcloud-dav` | `DavClient::mkcol(path)` | `&DavPath` | `()` |
| `/remote.php/dav/files/{user}/{path}` | `DELETE` | `nextcloud-dav` | `DavClient::delete(path)` | `&DavPath` | `()` |
| `/remote.php/dav/files/{user}/{path}` | `COPY` | `nextcloud-dav` | `DavClient::copy(src, dst, overwrite)` | `&DavPath`, `&DavPath`, `Overwrite` | `()` |
| `/remote.php/dav/files/{user}/{path}` | `MOVE` | `nextcloud-dav` | `DavClient::move_(src, dst, overwrite)` | `&DavPath`, `&DavPath`, `Overwrite` | `()` |

### OCS file sharing

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares` | `GET` | `nextcloud-shares` | `ShareClient::list(path)` | `Option<&str>` | `Vec<Share>` |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares/{id}` | `GET` | `nextcloud-shares` | `ShareClient::get(share_id)` | `&str` | `Share` |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares` | `POST` | `nextcloud-shares` | `ShareClient::create(request)` | `&CreateShare` | `Share` |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares/{id}` | `DELETE` | `nextcloud-shares` | `ShareClient::delete(share_id)` | `&str` | `()` |

### Notifications v2

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/apps/notifications/api/v2/notifications` | `GET` | `nextcloud-notifications` | `NotificationClient::list()` | — | `Vec<Notification>` |
| `/ocs/v2.php/apps/notifications/api/v2/notifications/{id}` | `GET` | `nextcloud-notifications` | `NotificationClient::get(id)` | `u64` | `Notification` |
| `/ocs/v2.php/apps/notifications/api/v2/notifications/{id}` | `DELETE` | `nextcloud-notifications` | `NotificationClient::delete(id)` | `u64` | `()` |
| `/ocs/v2.php/apps/notifications/api/v2/notifications` | `DELETE` | `nextcloud-notifications` | `NotificationClient::delete_all()` | — | `()` |

### Activity v2

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/apps/activity/api/v2/activity` | `GET` | `nextcloud-activity` | `ActivityClient::list(filter)` | `&ActivityFilter` | `Vec<Activity>` |
| `/ocs/v2.php/apps/activity/api/v2/activity` | `GET` | `nextcloud-activity` | `ActivityClient::list_for_object(object_type, object_id)` | `&str`, `u64` | `Vec<Activity>` |

### Chunked uploads v2

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/remote.php/dav/uploads/{user}/{session_id}` | `MKCOL` | `nextcloud-uploads` | `UploadClient::start(user_id, destination_path, total_size)` / `start_with_id(user_id, destination_path, total_size, upload_id)` | `&str`, `&str`, `u64`, `&str` | `UploadSession` |
| `/remote.php/dav/uploads/{user}/{session_id}/{chunk}` | `PUT` | `nextcloud-uploads` | `UploadClient::upload_chunk(&session, chunk_index, chunk_data)` | `&UploadSession`, `u32`, `Vec<u8>` | `()` |
| `/remote.php/dav/uploads/{user}/{session_id}/.file` | `MOVE` | `nextcloud-uploads` | `UploadClient::finish(&session, mtime)` | `&UploadSession`, `Option<u64>` | `()` |
| `/remote.php/dav/uploads/{user}/{session_id}` | `DELETE` | `nextcloud-uploads` | `UploadClient::abort(&session)` | `&UploadSession` | `()` |

`UploadSession` carries `user_id`, `upload_id`, `destination_path`, `total_size`
and the set of already-uploaded chunk indexes; `upload_chunk` marks a chunk
uploaded in the session on success.

Helpers: `chunk_boundaries(total, chunk_size)` → `Vec<ChunkBoundary>`,
`expected_chunk_count()`, `is_chunk_uploaded(offset)`, `chunk_path(offset)`,
`assembly_path()`, `abort_path()`.

### Previews & thumbnails

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/index.php/core/preview.png` | `GET` | `nextcloud-preview` | `PreviewClient::get_by_file_id(file_id, options)` | `u64`, `&PreviewOptions` | `Vec<u8>` |
| `/index.php/core/preview.png` | `GET` | `nextcloud-preview` | `PreviewClient::get_by_path(path, options)` | `&str`, `&PreviewOptions` | `Vec<u8>` |

Paths are built as `ApiPath::public("core/preview.png?…")` — i.e. `/index.php`-rooted,
not `/core/preview.png`. On a `400`/`404` the client retries once against the same
endpoint with the `.png` extension flipped, for servers whose front-end proxies
intercept static-looking paths.

The crate also owns the FreeDesktop thumbnail protocol:
`freedesktop_thumbnail_dirs()`, `install_freedesktop_thumbnail[_with_mtime]()`,
`install_freedesktop_fail_marker()` / `remove_freedesktop_fail_marker()`,
`path_to_file_uri()`, `crc32()`, `md5_hex()`, `ensure_png_bytes()`,
`embed_freedesktop_metadata()`, `extract_embedded_jpeg_thumbnail()`.

### Trashbin & versions

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/remote.php/dav/trashbin/{user}/trash` | `PROPFIND` | `nextcloud-trashbin` | `TrashClient::list(user_id)` | `&str` | `Vec<TrashItem>` |
| `/remote.php/dav/trashbin/{user}/trash/{id}` | `MOVE` | `nextcloud-trashbin` | `TrashClient::restore(user_id, item_id, restore_path)` | `&str`, `&str`, `&str` | `()` |
| `/remote.php/dav/trashbin/{user}/trash/{id}` | `DELETE` | `nextcloud-trashbin` | `TrashClient::delete(user_id, item_id)` | `&str`, `&str` | `()` |
| `/remote.php/dav/trashbin/{user}/trash` | `DELETE` | `nextcloud-trashbin` | `TrashClient::empty(user_id)` | `&str` | `()` |
| `/remote.php/dav/versions/{user}/versions/{id}` | `PROPFIND` | `nextcloud-versions` | `VersionsClient::list(user_id, file_id)` | `&str`, `u64` | `Vec<FileVersion>` |
| `/remote.php/dav/versions/{user}/versions/{id}/{rev}` | `GET` | `nextcloud-versions` | `VersionsClient::get(user_id, file_id, revision_id)` | `&str`, `u64`, `&str` | `Vec<u8>` |
| `/remote.php/dav/versions/{user}/versions/{id}/{rev}` | `MOVE` | `nextcloud-versions` | `VersionsClient::restore(user_id, file_id, revision_id)` | `&str`, `u64`, `&str` | `()` |

Every trashbin and versions call takes the target `user_id` as its first argument —
the client does not read it from the session.

### Deck (Kanban)

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/index.php/apps/deck/api/v1.0/boards` | `GET` | `nextcloud-deck` | `DeckClient::list_boards()` | — | `Vec<Board>` |
| `/index.php/apps/deck/api/v1.0/boards/{id}` | `GET` / `POST` / `PUT` / `DELETE` | `nextcloud-deck` | `get_board` / `create_board` / update / `delete_board` | `u64`, title, … | `Board` |
| `/index.php/apps/deck/api/v1.0/boards/{id}/stacks` | `GET` / `POST` / `DELETE` | `nextcloud-deck` | `list_stacks` / `create_stack` / `delete_stack` | `board_id: u64` | `Vec<Stack>` |
| `/index.php/apps/deck/api/v1.0/boards/{id}/stacks/{sid}/cards` | `GET` / `POST` | `nextcloud-deck` | `list_cards` / `create_card` | `u64`, `u64`, `&str` | `Vec<Card>` / `Card` |
| `/index.php/apps/deck/api/v1.0/boards/{id}/stacks/{sid}/cards/{cid}` | `DELETE` | `nextcloud-deck` | `delete_card(...)` | ids | `()` |

### Talk (Spreed)

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/apps/spreed/api/v4/room` | `GET` | `nextcloud-talk` | `TalkClient::list_rooms()` | — | `Vec<Room>` |
| `/ocs/v2.php/apps/spreed/api/v4/room` | `POST` | `nextcloud-talk` | `TalkClient::create_room(...)` | room type, name | `Room` |
| `/ocs/v2.php/apps/spreed/api/v4/room/{token}` | `GET` / `DELETE` | `nextcloud-talk` | `get_room` / `delete_room` | `&str` | `Room` |
| `/ocs/v2.php/apps/spreed/api/v1/chat/{token}` | `GET` / `POST` | `nextcloud-talk` | `list_messages` / `send_message` | `&str`, `&str` | `Vec<ChatMessage>` / `ChatMessage` |

### Unified search

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/ocs/v2.php/search/providers` | `GET` | `nextcloud-search` | `SearchClient::list_providers()` | — | `Vec<SearchProvider>` |
| `/ocs/v2.php/search/providers/{id}/search` | `GET` | `nextcloud-search` | `SearchClient::search(provider_id, term)` / `search_with_options(provider_id, term, limit, cursor)` | `&str`, `&str`, `Option<u32>`, `Option<i64>` | `SearchResultSet` |

Search is always scoped to one provider — there is no global query method. Call
`list_providers()` first, then `search(&provider.id, term)`.

### Theming & branding

| Endpoint | Method | Crate | Client / method | Parameters | Returns |
| --- | --- | --- | --- | --- | --- |
| `/apps/theming/api/v1/theme` | `GET` | `nextcloud-theming` | `ThemingClient::get_theme()` | — | `ThemeInfo` |
| theme `logo_header`/`logo`/`favicon` URL | `GET` | `nextcloud-theming` | `ThemingClient::fetch_logo_bytes()` | — (resolves the path from the theme) | `Vec<u8>` |
| `/status.php` + theme | `GET` | `nextcloud-theming` | `ThemingClient::effective_branding(custom)` | `Option<&CustomBranding>` | `EffectiveBranding` |

`EffectiveBranding` merges server theming with local `CustomBranding` overrides
(`app_name`, `instance_name`, `color`, `color_text`, `color_element`, `logo`,
`background`, `background_url`, `favicon`, `build_user_agent`).

### OS account sources (SSO & migration)

These read local system state instead of HTTP endpoints, so their "endpoint" is a
D-Bus bus name, a SQLite file or an INI file.

| Source | Access | Crate | Client / method | Returns |
| --- | --- | --- | --- | --- |
| `org.gnome.OnlineAccounts` | D-Bus (`zbus`) | `nextcloud-goa` | `GnomeOnlineAccounts::connect()` → `list_accounts()` | `Vec<GoaAccount>` |
| `~/.local/share/accounts/accounts.db` | SQLite | `nextcloud-kaccounts` | `KAccountsDatabase::open_default()` → `list_accounts()`; free function `find_kaccount_credentials()` | `Vec<KAccount>` |
| `nextcloud.cfg` (Linux/macOS/Windows) | INI | `nextcloud-client-config` | `ClientConfigParser::parse_file(path)` / `parse_str(&str)`; `discover_config_paths()`, `find_active_config_path()` | `Vec<DesktopClientAccount>` |
| All of the above | Discovery | `nextcloud-rust-sdk` (`sso`) | `DesktopSso::discover_accounts()` → `Vec<DiscoveredAccount>`; `resolve_credentials()` | `Vec<DiscoveredAccount>` |

---

## 3. Crate-by-crate reference

### 3.1 `nextcloud-sdk-core` (core)

Foundational HTTP semantics, type-safe paths, and the two seam traits.

- `ServerUrl` — validated base URL.
- `ApiPath` — relative endpoint appended to `ServerUrl`.
- `Transport` — `async fn execute(&self, req: Request) -> Result<Response, Error>`.
- `AuthProvider` — injects credentials into outbound `Headers`.
- `Header` / `Headers` — case-insensitive map with credential redaction.
- `Error` — unified error covering URL parsing, transport failures, non-2xx
  status (`StatusError`), and JSON/XML decode (`DecodeError`).

### 3.2 `nextcloud-sdk-session` (core)

Pure Login Flow v2 state machine; performs no I/O.

- `SessionState::{ Idle, AwaitingLogin(LoginFlow), AwaitingGrant(LoginFlow), Authenticated(Account), ReauthRequired(Account) }`
- `LoginStart { login_url, poll_endpoint, token }` with accessors of the same names.
- `LoginSuccess { server, login_name, app_password }`.
- `SessionEvent` in, `Command` out — the caller performs the described effect.

### 3.3 `nextcloud-sdk-request` (core)

Pure per-request retry/backoff state machine.

- `RequestMachine` with `RequestState::{ Idle, InFlight, RetryScheduled, Succeeded, Failed, Cancelled }`.
- `RetryPolicy` — attempt count, base delay, retryable status codes.
- `Effect::{ Send, Wait, Finish, GiveUp }` and `Failure` classification.

### 3.4 `nextcloud-sdk-storage` (core)

SQLCipher SQLite persistence for three databases.

- `StoragePaths` — `auth.db`, `cache.db`, `config.db` locations
  (`default_xdg()`, `ensure_directories()`).
- `StoreKey` — 256-bit key from OS entropy.
- `EncryptedDatabase` — schema verification and migrations.
- `KeyStore` trait with `KeychainKeyStore` and the platform keyring backends.

Details, including the schema per database, are in
[`SECURITY_AND_STORAGE.md`](SECURITY_AND_STORAGE.md).

### 3.5 `nextcloud-sdk-keyring` (core)

- `KeyringBackend` — trait for storing/retrieving the `StoreKey`.
- `PlatformKeyring` — FreeDesktop Secret Service / Flatpak portal (`oo7`) on Linux,
  Keychain on macOS, Credential Manager on Windows.
- `MemoryKeyring` — tests and ephemeral sessions.
- `KeyringEntry`, `KeyringError`.

### 3.6 `nextcloud-sdk-ratelimit` (core)

- `BandwidthLimit::{ Unlimited, BytesPerSecond, from_kib, from_mib }`.
- `TokenBucket` — thread-safe, async token acquisition, runtime rate changes.
- `BandwidthLimiter` — separate upload and download shaping.
- `TransferProgress { transferred_bytes, total_bytes, fraction(), percentage(), is_complete() }`.
- `ProgressCallback` — closure invoked per transfer; a mount-wide aggregate is
  `MountProgressSnapshot`, not this.

### 3.7 `nextcloud-sdk-vfs` (core)

The data a file browser renders, with no UI toolkit dependency.

- `VirtualTree` (hierarchy, thumbnails, navigation), `VirtualNode`, `NodeMetadata`.
- `HydrationState` / `HydratedContent` — `Placeholder` vs hydrated bytes/path.
- `NextcloudPermissions` (decoded bitfield), `ResourceIconCategory`.
- `VirtualEntrySummary`, `DirectorySizeSummary`.
- `FileSortField`, `FileSortOrder`, `FileSortCriteria`.
- `NavigationHistory`, `BreadcrumbSegment`, `breadcrumb_segments()`, `normalize_path()`.
- `ThumbnailCache` — multi-resolution storage.
- `HierarchySeeder` / `StandardNextcloudSeeder`, `DavResourceSummary`.

### 3.8 `nextcloud-sdk-fuse` (core)

- `mount_virtual_fs(driver, mountpoint)` and
  `mount_virtual_fs_with_options(driver, mountpoint, options)` → `BackgroundSession`.
  Driver comes first; the plain form is read-only.
- `FuseDriver::{ new, with_hydrator, with_hydrator_and_root }`.
- `AsyncHydrator` trait, `InodeTable`, `FileAttr`, `DirEntry`, `FileType`,
  `MountOption`, `parse_date_to_unix_secs`.
- Linux only; other targets return `io::ErrorKind::Unsupported`.

### 3.9 `nextcloud-sdk-orchestrator` (core)

The runtime. Owns an OS thread and its own Tokio runtime.

- `NextcloudOrchestrator::start(OrchestratorConfig)` → `.handle()`, `.shutdown()`.
- `OrchestratorConfig { base_cache_dir, mounts, sync_interval_seconds, worker_threads }`
  with `new`, `with_mount`, `with_sync_interval_seconds`, `with_worker_threads`.
- `OrchestratorHandle` — `mounts()`, `vfs(mount_id)`, `subscribe_events()`,
  `pause_all_sync()`, `resume_all_sync()`, `is_sync_paused()`, `shutdown()`.
- `MountsHandle` — `add_mount`, `remove_mount`, `list_mounts`, `get_mount`,
  `sync_mount` *(local reconciliation against the journal — no DAV round-trip)*,
  `pause_mount`, `resume_mount`, `is_paused`, `storage_usage`, `clear_cache`,
  `clear_thumbnail_cache`, `progress`, `file_status`, `status_for_local_path`,
  `set_file_status`.
- `FolderSyncRunner` — `new(local_dir, remote_dir, journal_path)`,
  `discover_remote(dav_client, user, remote_root)`,
  `plan_reconciliation(&[DiscoveredRemoteFile], is_vfs) -> (Vec<SyncAction>, SyncJournal)`,
  `execute_sync_cycle(dav_client, user, is_vfs, is_cancelled, on_progress) -> FolderSyncOutcome`,
  `run_local_reconciliation()` (journal vs local dir, no network), plus
  `hydrate_file_from_dav`, `upload_file_to_dav`, `create_remote_directory`,
  `move_remote_resource`, `delete_remote_resource`. `execute_sync_cycle` is the
  real network cycle; nothing in the orchestrator schedules it for you.
- `VfsHandle` — `list_children`, `snapshot`, `entry`, `populate_from_resources`,
  `is_directory_populated`, `unpopulated_directories`, `hydrate_local`,
  `dehydrate_local`, `create_directory`, `rename`, `remove`.
- `OrchestratorEvent` — the runtime event vocabulary (see `ARCHITECTURE.md`).
- `FileSystemWatcher`, `DownloadQueueCoordinator`, `VfsManager`, `make_dav_path`,
  `SyncFileStatus`, `MountStorageUsage`, `MountProgressSnapshot`.

### 3.10 `nextcloud-auth`

- `BasicAuth`, `BearerAuth`, `Anonymous` (adds `OCS-APIRequest: true`).
- `SecretString` — redacting wrapper used for app passwords.

### 3.11 `nextcloud-transport`

- `ReqwestTransport` implementing `Transport`, with `set_upload_limit`,
  `set_download_limit`, `set_download_progress`, `set_upload_progress`,
  `execute_with_progress`.
- `TransportBuilder` — server URL, timeouts, limits, progress callbacks.
- `TimeoutConfig`, `TransportBuildError`.

### 3.12 `nextcloud-ocs`

- `OcsEnvelope<T>`, `Ocs<T>`, `Meta`, `OcsStatus`, `OcsFormat`, `Query`.
- `OcsClient<T, A>` — typed `get`, `post`, `put`, `delete` with the mandatory
  `OCS-APIRequest: true` and `Accept: application/json` headers.

### 3.13 `nextcloud-dav`

- `DavClient<'a, T, A>` — `propfind`, `get`, `get_range`, `put`, `mkcol`, `delete`,
  `copy`, `move_`.
- `DavPath` (`DavPath::new(user, relative)`), `DavProperty`.
- `DavResource`, `Depth::{Zero, One, Infinity}`, `Overwrite::{Allow, Forbidden}`.
- `PropfindBody::{ new(properties), all() }` — `new` takes `impl IntoIterator<Item = DavProperty>`; there is no `names`/`prop` constructor.

### 3.14 `nextcloud-users`

- `UserClient<'a, T, A>` — `capabilities`, `current_user`, `get_user`.
- `Capabilities::server_version()`, `version()`, `status()`, `theming()`, `core()`, `files()`.
- `CoreCapabilities::webdav_root()`, `FilesCapabilities::{ chunked_upload, max_chunk_size }`.
- `User`, `Quota`, `ServerVersion`, `ServerStatus`, `ThemingCapabilities`.

### 3.15 `nextcloud-shares`

- `ShareClient<'a, T, A>` — `list`, `get`, `create`, `delete`.
- `CreateShare` — public link / user / group builders.
- `Share`, `SharePermissions` (bitflags), `ShareType`.

### 3.16 `nextcloud-notifications`

- `NotificationClient<'a, T, A>` — `list`, `get`, `delete`, `delete_all`.
- `Notification`, `NotificationAction`.

### 3.17 `nextcloud-activity`

- `ActivityClient<'a, T, A>` — `list`, `list_for_object`.
- `ActivityFilter` — `limit`, `since`, `object_type`, `object_id`, `sort`.

### 3.18 `nextcloud-sync`

The reconciliation engine *behind* the orchestrator. Applications normally never
call it directly.

- `reconcile`, `SyncAction`, `SyncJournal`, `JournalEntry`, `scan_local_directory`.
- `DiscoveredLocalFile`, `DiscoveredRemoteFile`, `SyncError`.
- `SyncDownloadQueue` + `DownloadTask` / `DownloadTaskId` / `DownloadPriority` /
  `DownloadStatus` / `DownloadQueueEvent` / `DownloadQueueSnapshot`,
  `DownloadExecutor`, `DownloadProgress`.

### 3.19 `nextcloud-theming`

- `ThemingClient<'a, T, A>` — `get_theme`, `server_status`, `fetch_logo_bytes`,
  `effective_branding`.
- `ThemeInfo`, `CustomBranding`, `EffectiveBranding`, `ServerStatusInfo`, `ThemingError`.

### 3.20 `nextcloud-preview`, `nextcloud-trashbin`, `nextcloud-versions`, `nextcloud-uploads`, `nextcloud-deck`, `nextcloud-talk`, `nextcloud-search`

See the endpoint tables above; each crate exposes exactly the methods listed, plus
value types named after their row (`PreviewOptions`, `TrashItem`, `FileVersion`,
`UploadSession`, `Board` / `Stack` / `Card`, `Room` / `ChatMessage`,
`SearchProvider` / `SearchEntry` / `SearchResultSet`).

### 3.21 `nextcloud-goa`, `nextcloud-kaccounts`, `nextcloud-client-config`

OS-level account discovery for SSO and migration: GNOME Online Accounts and
KAccounts over their native channels, plus `nextcloud.cfg` parsing so a consumer
can adopt existing account and folder pairs on first run.

### 3.22 Facade: `nextcloud-rust-sdk`

- `ClientBuilder` — `server_url`, `storage`/`storage_paths`, `store_key`,
  `key_store`, `keychain(app_id, account)`, `initial_account`,
  `initial_credentials`, `retry_policy`, `timeout_config`, `upload_limit`,
  `download_limit`, `on_download_progress`, `on_upload_progress`, `branding`,
  `build()`.
- `Client` — session (`start_login`, `poll_login`, `logout`, `clear_cache`,
  `credentials`, `session_state`, `is_authenticated`, `current_user_id`,
  `server_status`), client factories (see §2), persistence
  (`store_sync_mount`, `remove_sync_mount`, `list_sync_mounts`,
  `store_cached_theme`, `load_cached_theme`, `store_directory_sort`,
  `load_directory_sort`, `set_setting`, `get_setting`), runtime knobs
  (`set_upload_limit`, `set_download_limit`, `set_download_progress`,
  `set_upload_progress`, `subscribe_events`).
- `ClientEvent::{ SyncMountAdded, SyncMountRemoved, SyncMountUpdated, ThemeUpdated }`
  — persisted configuration changes, complementary to `OrchestratorEvent`.
- `StoredSyncMount`, `CachedTheme`, `CachedDavEntry`, `ConfigStore`, `SessionStore`,
  `CacheStore`, `WorkerActor`, `WorkerTask`, `WorkerStores`.
