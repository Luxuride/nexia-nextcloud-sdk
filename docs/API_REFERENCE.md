# Nextcloud Rust SDK — API Reference & Endpoint Matrix

This document provides a comprehensive mapping between Nextcloud server HTTP endpoints, the implementing Rust SDK crates, their strongly typed client methods, input parameters, and response structures.

---

## 1. Official Nextcloud Developer Documentation

All client modules in this SDK are built against the official Nextcloud developer specifications:
- [Nextcloud Developer Manual (GitHub master)](https://github.com/nextcloud/documentation/tree/master/developer_manual)
- [Login Flow v2 Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/LoginFlow/index.rst)
- [OCS API Overview & Protocol Guidelines](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-api-overview.rst)
- [WebDAV Endpoints & Multi-Status Protocol](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/index.rst)
- [OCS Share API Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst)
- [Notifications API v2 Specification](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md)
- [Activity API v2 Specification](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst)
- [User Provisioning API](https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html)

---

## 2. API Endpoint Matrix

| Endpoint Path | HTTP Method | SDK Crate | Rust Client / Method | Parameters | Return Type | Official Documentation |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Authentication & Login** | | | | | | |
| `/index.php/login/v2` | `POST` | `sdk-session` / `sdk-core` | `SessionMachine::handle` | `SessionEvent::StartRequested` | `Command::StartLogin` | [Login Flow v2](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/LoginFlow/index.rst) |
| `/login/v2/poll` | `POST` | `sdk-session` / `sdk-core` | `SessionMachine::handle` | `SessionEvent::PollPending` | `Command::Poll(PollTarget)` | [Poll Endpoint](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/LoginFlow/index.rst#polling) |
| **Server Capabilities & Users** | | | | | | |
| `/ocs/v2.php/cloud/capabilities` | `GET` | `nextcloud-users` | `UserClient::capabilities()` | None (`format=json`) | `Capabilities` | [Capabilities API](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-api-overview.rst#capabilities-api) |
| `/ocs/v2.php/cloud/user` | `GET` | `nextcloud-users` | `UserClient::current_user()` | None (`format=json`) | `User` | [User Metadata](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-api-overview.rst#user-metadata) |
| `/ocs/v2.php/cloud/users/{id}` | `GET` | `nextcloud-users` | `UserClient::get_user()` | `user_id: &str` | `User` | [User Provisioning](https://docs.nextcloud.com/server/latest/admin_manual/configuration_user/user_provisioning_api.html) |
| **WebDAV File Storage** | | | | | | |
| `/remote.php/dav/files/{user}/{path}` | `PROPFIND` | `nextcloud-dav` | `DavClient::propfind()` | `path: &DavPath`, `depth: Depth`, `body: &PropfindBody` | `Vec<DavResource>` | [WebDAV Finding Properties](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `GET` | `nextcloud-dav` | `DavClient::get()` | `path: &DavPath` | `Vec<u8>` | [WebDAV Downloading Files](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `PUT` | `nextcloud-dav` | `DavClient::put()` | `path: &DavPath`, `body: Vec<u8>`, `content_type: Option<&str>` | `()` | [WebDAV Uploading Files](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `MKCOL` | `nextcloud-dav` | `DavClient::mkcol()` | `path: &DavPath` | `()` | [WebDAV Creating Folders](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `DELETE` | `nextcloud-dav` | `DavClient::delete()` | `path: &DavPath` | `()` | [WebDAV Deleting Files/Folders](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `COPY` | `nextcloud-dav` | `DavClient::copy()` | `src: &DavPath`, `dst: &DavPath`, `overwrite: Overwrite` | `()` | [WebDAV Copying](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| `/remote.php/dav/files/{user}/{path}` | `MOVE` | `nextcloud-dav` | `DavClient::move_()` | `src: &DavPath`, `dst: &DavPath`, `overwrite: Overwrite` | `()` | [WebDAV Moving](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/basic.rst) |
| **OCS File Sharing** | | | | | | |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares` | `GET` | `nextcloud-shares` | `ShareClient::list()` | `path: Option<&str>` | `Vec<Share>` | [Get All Shares](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst#get-all-shares) |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares/{id}` | `GET` | `nextcloud-shares` | `ShareClient::get()` | `share_id: &str` | `Share` | [Get Known Share](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst#get-information-about-a-known-share) |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares` | `POST` | `nextcloud-shares` | `ShareClient::create()` | `request: &CreateShare` | `Share` | [Create a New Share](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst#create-a-new-share) |
| `/ocs/v2.php/apps/files_sharing/api/v1/shares/{id}` | `DELETE` | `nextcloud-shares` | `ShareClient::delete()` | `share_id: &str` | `()` | [Delete Share](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-share-api.rst#delete-share) |
| **Notifications API v2** | | | | | | |
| `/ocs/v2.php/apps/notifications/api/v2/notifications` | `GET` | `nextcloud-notifications` | `NotificationClient::list()` | None | `Vec<Notification>` | [List Notifications](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#getting-the-notifications-of-a-user) |
| `/ocs/v2.php/apps/notifications/api/v2/notifications/{id}` | `GET` | `nextcloud-notifications` | `NotificationClient::get()` | `id: u64` | `Notification` | [Get Notification](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#get-a-single-notification-for-a-user) |
| `/ocs/v2.php/apps/notifications/api/v2/notifications/{id}` | `DELETE` | `nextcloud-notifications` | `NotificationClient::delete()` | `id: u64` | `()` | [Delete Notification](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#deleting-a-notification-for-a-user) |
| `/ocs/v2.php/apps/notifications/api/v2/notifications` | `DELETE` | `nextcloud-notifications` | `NotificationClient::delete_all()` | None | `()` | [Delete All Notifications](https://github.com/nextcloud/notifications/blob/master/docs/ocs-endpoint-v2.md#deleting-all-notifications-for-a-user) |
| **Activity Feed API v2** | | | | | | |
| `/ocs/v2.php/apps/activity/api/v2/activity` | `GET` | `nextcloud-activity` | `ActivityClient::list()` | `filter: &ActivityFilter` | `Vec<Activity>` | [List Activities](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst#endpoints) |
| `/ocs/v2.php/apps/activity/api/v2/activity` | `GET` | `nextcloud-activity` | `ActivityClient::list_for_object()` | `object_type: &str`, `object_id: u64` | `Vec<Activity>` | [Object Activities](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst#parameters) |
| **Chunked Uploads v2** | | | | | | |
| `/remote.php/dav/uploads/{user}/{session_id}` | `MKCOL` | `nextcloud-uploads` | `UploadClient::start()` | `session_id: &str` | `UploadSession` | [Chunked Uploads v2](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/chunking.rst) |
| `/remote.php/dav/uploads/{user}/{session_id}/{offset}` | `PUT` | `nextcloud-uploads` | `UploadClient::put_chunk()` | `chunk: &[u8]`, `offset: u64` | `()` | [Chunk Transfer](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/chunking.rst) |
| `/remote.php/dav/uploads/{user}/{session_id}/.file` | `MOVE` | `nextcloud-uploads` | `UploadClient::finish()` | `destination: &DavPath` | `()` | [Assemble Chunks](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/chunking.rst) |
| `/remote.php/dav/uploads/{user}/{session_id}` | `DELETE` | `nextcloud-uploads` | `UploadClient::abort()` | None | `()` | [Abort Session](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/chunking.rst) |
| **Nextcloud Deck (Kanban)** | | | | | | |
| `/index.php/apps/deck/api/v1.0/boards` | `GET` | `nextcloud-deck` | `DeckClient::list_boards()` | None | `Vec<Board>` | [Deck API](https://deck.readthedocs.io/en/latest/API/) |
| `/index.php/apps/deck/api/v1.0/boards/{id}/stacks` | `GET` | `nextcloud-deck` | `DeckClient::list_stacks()` | `board_id: u64` | `Vec<Stack>` | [Deck Stacks](https://deck.readthedocs.io/en/latest/API/) |
| `/index.php/apps/deck/api/v1.0/boards/{id}/stacks/{sid}/cards` | `POST` | `nextcloud-deck` | `DeckClient::create_card()` | `title: &str`, `type_: &str` | `Card` | [Deck Cards](https://deck.readthedocs.io/en/latest/API/) |
| **Nextcloud Talk (Spreed)** | | | | | | |
| `/ocs/v2.php/apps/spreed/api/v4/room` | `GET` | `nextcloud-talk` | `TalkClient::list_rooms()` | None | `Vec<Room>` | [Talk API](https://nextcloud-talk.readthedocs.io/en/latest/chat/) |
| `/ocs/v2.php/apps/spreed/api/v1/chat/{token}` | `GET` | `nextcloud-talk` | `TalkClient::list_messages()` | `token: &str` | `Vec<ChatMessage>` | [Talk Chat](https://nextcloud-talk.readthedocs.io/en/latest/chat/) |
| `/ocs/v2.php/apps/spreed/api/v1/chat/{token}` | `POST` | `nextcloud-talk` | `TalkClient::send_message()` | `token: &str`, `message: &str` | `ChatMessage` | [Send Message](https://nextcloud-talk.readthedocs.io/en/latest/chat/) |
| **Previews & Thumbnails** | | | | | | |
| `/core/preview.png` | `GET` | `nextcloud-preview` | `PreviewClient::get()` | `file_id: u64`, `options: &PreviewOptions` | `Vec<u8>` | [Previews](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/OCS/ocs-api-overview.rst) |
| **Trashbin & Versions** | | | | | | |
| `/remote.php/dav/trashbin/{user}/trash` | `PROPFIND` | `nextcloud-trashbin` | `TrashClient::list()` | None | `Vec<TrashItem>` | [Trashbin](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/trashbin.rst) |
| `/remote.php/dav/versions/{user}/versions/{id}` | `PROPFIND` | `nextcloud-versions` | `VersionsClient::list()` | `file_id: u64` | `Vec<FileVersion>` | [Versions](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/WebDAV/versions.rst) |
| **Desktop SSO Integration** | | | | | | |
| `org.gnome.OnlineAccounts` (D-Bus) | `GetManagedObjects` | `nextcloud-goa` | `GnomeOnlineAccounts::list_accounts()` | None | `Vec<GoaAccount>` | [GOA](https://developer.gnome.org/goa/) |
| `~/.local/share/accounts/accounts.db` | `SELECT` | `nextcloud-kaccounts` | `KAccountsDatabase::list_accounts()` | None | `Vec<KAccount>` | [KAccounts](https://invent.kde.org/frameworks/kaccounts-integration) |
| `nextcloud.cfg` (Linux/macOS/Windows) | INI parser | `nextcloud-client-config` | `ClientConfigParser::parse_file()` | None | `Vec<DesktopClientAccount>` | [Client Config](https://docs.nextcloud.com/desktop/latest/configuration.html) |
| Aggregated Desktop SSO | Discovery | `nextcloud-rust-sdk` | `DesktopSso::discover_accounts()` | None | `Vec<DiscoveredAccount>` | Nextcloud Rust SDK |

---

## 3. Crate Architecture Reference

### 3.1 `sdk-core`
- **Purpose**: Foundational HTTP semantics, type-safe path abstractions, and seam traits.
- **Key Types**:
  - `ServerUrl`: Validated base URL (`https://...`).
  - `ApiPath`: Relative endpoint path appended to `ServerUrl`.
  - `Transport`: Trait for asynchronous HTTP execution (`execute(&self, req) -> Result<Response, Error>`).
  - `AuthProvider`: Trait for credential injection into outbound `Headers`.
  - `Header` / `Headers`: Case-insensitive HTTP header map with automatic credential redaction.
  - `Error`: Unified error enum classifying URL parsing, network transport failures, non-2xx status codes, and JSON/XML decode errors.

### 3.2 `sdk-session`
- **Purpose**: Pure transition state machine governing account lifecycles and Login Flow v2.
- **States**:
  - `SessionState::Idle`: No account or active flow.
  - `SessionState::AwaitingLogin(LoginFlow)`: Flow initialized; awaiting browser confirmation.
  - `SessionState::AwaitingGrant(LoginFlow)`: Actively polling token endpoint (handling 404 responses).
  - `SessionState::Authenticated(Account)`: Active authenticated session with server URL and app password.
  - `SessionState::ReauthRequired(Account)`: Credentials were rejected (401); re-authentication necessary.

### 3.3 `sdk-request`
- **Purpose**: Pure transition state machine for per-request retry scheduling and exponential backoff.
- **Key Types**:
  - `RequestMachine`: Tracks attempt count and current state (`Idle`, `InFlight`, `RetryScheduled`, `Succeeded`, `Failed`, `Cancelled`).
  - `RetryPolicy`: Configurable maximum attempt counts, base delays, and retryable HTTP status codes.
  - `Effect`: Instruction to the caller (`Send`, `Wait`, `Finish`, `GiveUp`).

### 3.4 `sdk-storage`
- **Purpose**: Persistent encrypted storage using SQLCipher.
- **Key Types**:
  - `StoragePaths`: Configures disk locations for `auth.db` and `cache.db` (defaults to `$XDG_DATA_HOME/nextcloud-rust-sdk/`).
  - `StoreKey`: 256-bit cryptographic key generated via `getrandom`.
  - `KeychainKeyStore`: Platform keychain backend; uses `oo7` to communicate with the FreeDesktop Secret Service on Linux.
  - `EncryptedDatabase`: SQLCipher database wrapper with automatic migration and schema verification.
  - `CredentialSchema` / `CacheSchema`: Schema DDL definitions and versioning.

### 3.5 `sdk-ratelimit`
- **Purpose**: Dynamic bandwidth rate limiting, token-bucket traffic shaping, and transfer progress tracking.
- **Key Types**:
  - `BandwidthLimit`: Rate constraint (`Unlimited`, `BytesPerSecond`, `from_kib`, `from_mib`).
  - `TokenBucket`: Thread-safe, non-blocking token bucket algorithm with asynchronous token acquisition and runtime rate adjustment.
  - `BandwidthLimiter`: High-level traffic shaper coordinating independent upload and download limits on active connections.
  - `TransferProgress`: Progress snapshot (`transferred_bytes`, `total_bytes`, `fraction`, `percentage`, `is_complete`).
  - `ProgressCallback`: Thread-safe closure callback invoked with incremental progress events.

### 3.6 `nextcloud-auth`
- **Purpose**: Authentication schemes implementing `AuthProvider`.
- **Key Types**:
  - `BasicAuth`: HTTP Basic authentication (`Authorization: Basic base64(user:password)`).
  - `BearerAuth`: OAuth2 bearer tokens (`Authorization: Bearer token`).
  - `Anonymous`: Anonymous access; injects the mandatory `OCS-APIRequest: true` header.
  - `SecretString`: Secure string wrapper redacting values in `Debug` and preventing accidental printing.

### 3.7 `nextcloud-transport`
- **Purpose**: Production HTTP transport implementation backed by `reqwest` with dynamic rate limiting and progress reporting.
- **Key Types**:
  - `ReqwestTransport`: Implements `sdk_core::Transport`, supports dynamic upload/download rate limits (`set_upload_limit`, `set_download_limit`) and progress reporting (`set_download_progress`, `execute_with_progress`).
  - `TransportBuilder`: Fluent builder for configuring server URL, timeouts, rate limits, and progress callbacks.
  - `TimeoutConfig`: Connect, read, and global timeout settings.

### 3.7 `nextcloud-ocs`
- **Purpose**: OCS v1/v2 envelope parser and request executor.
- **Key Types**:
  - `OcsEnvelope<T>`: Deserializes the standard Nextcloud `{ ocs: { meta: ..., data: ... } }` wrapper.
  - `OcsClient<T, A>`: Client providing typed `get()`, `post()`, `put()`, and `delete()` operations with mandatory OCS headers (`OCS-APIRequest: true`, `Accept: application/json`).

### 3.8 `nextcloud-dav`
- **Purpose**: WebDAV file system operations and PROPFIND XML generation/parsing.
- **Key Types**:
  - `DavClient<'a, T, A>`: Client for PROPFIND, GET, PUT, MKCOL, DELETE, COPY, MOVE.
  - `DavPath`: Validated WebDAV path rooted in a user namespace (`DavPath::new("alice", "Photos/trip.jpg")`).
  - `DavResource`: Parsed multi-status entry detailing href, collection status, content length, content type, etag, and last modified timestamp.
  - `PropfindBody`: Construct properties query (`all()`, `names()`, or explicit `prop(...)`).

### 3.9 `nextcloud-users`
- **Purpose**: User provisioning and server capabilities discovery.
- **Key Types**:
  - `UserClient<'a, T, A>`: Queries capabilities, current user, and individual users.
  - `Capabilities`: Server features (WebDAV root, big file chunking, core status, theming).
  - `User`: User profile with quota metrics, group memberships, and display name.

### 3.10 `nextcloud-shares`
- **Purpose**: File sharing management via Nextcloud's OCS Share API.
- **Key Types**:
  - `ShareClient<'a, T, A>`: Lists, retrieves, creates, and deletes shares.
  - `CreateShare`: Builder for public link shares, user shares, and group shares.
  - `Share`: Information about an existing share (id, token, URL, permissions).
  - `SharePermissions`: Bitflags configuring read, update, create, delete, and share permissions.
  - `ShareType`: User (0), Group (1), Public Link (3), Email (4), etc.

### 3.11 `nextcloud-notifications`
- **Purpose**: User notification fetching and dismissal.
- **Key Types**:
  - `NotificationClient<'a, T, A>`: Fetches lists, single notifications, dismisses, or clears all notifications.
  - `Notification`: Parsed notification object with app name, subject, message, link, and available interactive actions.
  - `NotificationAction`: Primary and secondary action buttons associated with notifications.

### 3.12 `nextcloud-activity`
- **Purpose**: Audit logging and user activity streams.
- **Key Types**:
  - `ActivityClient<'a, T, A>`: Queries activity streams and object histories.
  - `ActivityFilter`: Filter builder supporting `limit`, `since`, `object_type`, `object_id`, and `sort`.
  - `Activity`: Activity entry with id, subject, message, timestamp, affected files, and originating user.
