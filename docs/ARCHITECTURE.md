# Nextcloud Rust SDK — Architecture

`nexia-nextcloud-sdk` is the full-featured Rust SDK for Nextcloud. It is not a thin
HTTP wrapper: it owns the parts of a client that are hard to get right — the
credential and cache stores, session and request state machines, the mount
registry, the synchronization engine, the virtual filesystem with hydration
lifecycle, thumbnails, bandwidth shaping, and the event stream that a UI renders.

The deliberate consequence: **applications built on this SDK are thin**. A GUI,
a headless daemon or a mobile shell asks the SDK to perform an operation and
renders the resulting events. It does not crawl PROPFIND trees, reconcile mtimes,
keep journals or schedule retries itself.

The inverse is a design rule: **the SDK never acknowledges a specific
application.** No file here branches on "is this client X", the API carries no
consumer-specific fields, and no downstream product's config format, UI state or
naming is a dependency. The only inputs are Nextcloud's protocols and whatever
the caller passes in a descriptor or config struct.

---

## 1. Layers

```text
+----------------------------------------------------------------------------------+
| Consumer (thin) — any app: GUI, daemon, CLI, service; unknown to the SDK          |
|   holds handles -> sends commands -> renders events                              |
+----------------------------------------------------------------------------------+
              | commands (mpsc)                     ^ events (broadcast)
              v                                     |
+----------------------------------------------------------------------------------+
| nextcloud-sdk-orchestrator  (stateful runtime, dedicated OS thread)               |
|   NextcloudOrchestrator                                                          |
|   mount manager | folder sync runner | filesystem watcher | download queue |     |
|   VFS manager   | thumbnail cache    | sync status classification                |
+----------------------------------------------------------------------------------+
              | uses
              v
+----------------------------------------------------------------------------------+
| Engine crates (stateful, non-runtime)                                            |
|   nextcloud-sync      reconcile / SyncAction / SyncJournal / scanner / queue      |
|   nextcloud-sdk-vfs   VirtualTree / VirtualNode / HydrationState / ThumbnailCache |
|   nextcloud-sdk-fuse  FuseDriver over a VirtualTree + AsyncHydrator               |
|   nextcloud-sdk-storage  auth.db / cache.db / config.db (SQLCipher)               |
|   nextcloud-sdk-keyring  StoreKey persistence per platform                        |
|   nextcloud-sdk-ratelimit  token bucket + progress callbacks                      |
|   nextcloud-sdk-session    Login Flow v2 state machine                            |
|   nextcloud-sdk-request    per-call retry state machine                           |
+----------------------------------------------------------------------------------+
              | uses
              v
+----------------------------------------------------------------------------------+
| Protocol crates (stateless)                                                      |
|   nextcloud-dav | nextcloud-ocs | nextcloud-shares | nextcloud-users | ...        |
|   nextcloud-goa | nextcloud-kaccounts | nextcloud-client-config | nextcloud-theming |
|   nextcloud-trashbin | nextcloud-versions | nextcloud-uploads | nextcloud-preview  |
|   nextcloud-deck | nextcloud-talk | nexia-nextcloud-search | nextcloud-activity          |
+----------------------------------------------------------------------------------+
              | Transport + AuthProvider seams
              v
+----------------------------------------------------------------------------------+
| nextcloud-transport (reqwest)  /  nextcloud-auth (Basic, Bearer, Anonymous)       |
| nexia-nextcloud-core (ServerUrl, ApiPath, Header, Transport, AuthProvider, Error)   |
+----------------------------------------------------------------------------------+
```

The rule that keeps this honest: **protocol crates are stateless and borrow
their seams; runtime state lives only in the orchestrator.** A `DavClient` holds
no connection, token or lock. Anything that must remember something across calls
lives in `nextcloud-sdk-storage` or in the orchestrator worker.

---

## 2. Repository map

```text
nexia-nextcloud-sdk/
├── Cargo.toml                      # workspace: members = ["crates/*/*"]
├── src/                            # facade crate `nexia-nextcloud-sdk`
│   ├── lib.rs                      # re-exports + crate docs
│   ├── builder/client_builder.rs   # ClientBuilder
│   ├── client.rs                   # Client: session + client factories + mounts
│   ├── event.rs                    # ClientEvent
│   ├── sso/                        # DesktopSso discovery (GOA, KAccounts, nextcloud.cfg)
│   ├── storage/                    # ConfigStore, SessionStore, CacheStore, StoredSyncMount
│   └── worker/                     # Client actor wiring
├── docs/
│   ├── ARCHITECTURE.md             # this file
│   ├── GETTING_STARTED.md
│   ├── API_REFERENCE.md
│   └── SECURITY_AND_STORAGE.md
└── crates/
    ├── core/
    │   ├── nexia-nextcloud-core/         # Transport/AuthProvider seams, ServerUrl, ApiPath, Error
    │   ├── nextcloud-sdk-session/      # Login Flow v2 state machine
    │   ├── nextcloud-sdk-request/      # retry/backoff state machine
    │   ├── nextcloud-sdk-storage/      # encrypted stores
    │   ├── nextcloud-sdk-keyring/      # multi-platform keyring
    │   ├── nextcloud-sdk-ratelimit/    # bandwidth + progress
    │   ├── nextcloud-sdk-vfs/          # VirtualTree and hydration model
    │   ├── nextcloud-sdk-fuse/         # FUSE driver
    │   └── nextcloud-sdk-orchestrator/ # the runtime
    └── nextcloud/
        ├── nextcloud-auth/             # credential types
        ├── nextcloud-transport/        # reqwest transport
        ├── nextcloud-ocs/              # OCS envelope + mutation engine
        ├── nextcloud-dav/              # WebDAV CRUD + multistatus
        ├── nextcloud-users/            # capabilities, profiles
        ├── nextcloud-shares/           # sharing API
        ├── nextcloud-notifications/    # Notifications v2
        ├── nextcloud-activity/         # Activity v2
        ├── nextcloud-trashbin/         # trashbin
        ├── nextcloud-versions/         # file versions
        ├── nextcloud-uploads/          # chunked uploads v2
        ├── nextcloud-deck/             # Deck
        ├── nextcloud-talk/             # Talk
        ├── nextcloud-preview/          # previews/thumbnails
        ├── nextcloud-theming/          # theming, branding, /status.php
        ├── nexia-nextcloud-search/           # Unified Search
        ├── nextcloud-sync/             # reconciliation engine + journal + queue
        ├── nextcloud-goa/              # GNOME Online Accounts
        ├── nextcloud-kaccounts/        # KDE KAccounts
        └── nextcloud-client-config/    # nextcloud.cfg parser + discovery
```

---

## 3. The orchestrator: the stateful core

`nextcloud-sdk-orchestrator` is where the SDK stops being a library and becomes a
runtime. It is the only crate that owns an OS thread.

### 3.1 Lifecycle

```rust
use nextcloud_sdk_orchestrator::{NextcloudOrchestrator, OrchestratorConfig};

let orch = NextcloudOrchestrator::start(OrchestratorConfig::new(cache_dir))?;
let handle = orch.handle();               // cloneable, Send + Sync
let mut events = handle.subscribe_events();
```

`start` spawns an OS thread named `nextcloud-sdk-worker` (`worker.rs:33`) and
builds a **multi-threaded Tokio runtime** with `config.worker_threads`
(`worker.rs:36-38`), then runs the actor loop on it. The caller's own runtime is
never entered or blocked. `shutdown()` and `Drop` send a shutdown command and join
the thread. Command channel capacity is 256 and the event broadcast is 512
(`worker.rs:28-29`).

`OrchestratorConfig` (`config.rs`):

| Field | Meaning |
| --- | --- |
| `base_cache_dir` | Root for persisted tree, journals and thumbnails |
| `mounts: Vec<MountDescriptor>` | Initial mounts, restored at start |
| `sync_interval_seconds` | Intended periodic sync cadence, defaulting to `60` with `0` documented as "disable the timer" (`config.rs:17-18`, `:31`). Settable via `with_sync_interval_seconds`, but **no code reads the field** — `grep sync_interval_seconds crates/core/nextcloud-sdk-orchestrator/src/` matches only `config.rs`. Sync happens when a caller drives `sync_mount()` or `execute_sync_cycle`, or when a `FileSystemWatcher` the consumer constructed fires — never on a timer |
| `worker_threads` | Threads in the orchestrator's own Tokio runtime |

### 3.2 Command surface

Handles are the app's entire vocabulary. They are `Clone` and hold an
`mpsc::Sender<OrchestratorCommand>` plus a `broadcast::Sender<OrchestratorEvent>`.

`OrchestratorHandle`
: `mounts() -> MountsHandle`, `vfs(mount_id) -> VfsHandle`,
  `subscribe_events() -> broadcast::Receiver<OrchestratorEvent>`,
  `pause_all_sync()`, `resume_all_sync()`, `is_sync_paused() -> bool`,
  `shutdown()` — all async commands returning `OrchestratorResult<_>`.

`MountsHandle`
: `add_mount(MountDescriptor) -> MountId`, `remove_mount(MountId) -> MountDescriptor`,
  `list_mounts() -> Vec<(MountDescriptor, MountState)>`,
  `get_mount(id) -> Option<(MountDescriptor, MountState)>`, `sync_mount(id)`,
  `pause_mount(id)`, `resume_mount(id)`, `is_paused(id) -> bool`,
  `storage_usage(id) -> MountStorageUsage`, `clear_cache(id, clear_hydrated: bool) -> u64`,
  `clear_thumbnail_cache() -> u64`, `progress(id) -> MountProgressSnapshot`,
  `file_status(mount_id, relative_path) -> SyncFileStatus`,
  `status_for_local_path(local_path) -> Option<SyncFileStatus>`,
  `set_file_status(mount_id, relative_path, status)`. Every one of these is
  `async` and returns `OrchestratorResult<T>` — a failed command (`shutdown`,
  dropped worker) surfaces as `Err`, not as a panic.

`VfsHandle` (bound to one `MountId`; `mount_id()` returns it)
: `list_children(parent) -> Vec<VirtualEntrySummary>`, `snapshot() -> VirtualTree`,
  `entry(path) -> Option<VirtualEntrySummary>`,
  `populate_from_resources(dav_prefix, resources) -> usize` (inserted count; it
  merges into the tree, auto-creating intermediate parents, and prunes children of
  the populated directory that the server no longer reports — nodes outside that
  directory survive), `is_directory_populated(path) -> bool`,
  `unpopulated_directories() -> Vec<String>`, `hydrate_local(path, content) -> PathBuf`,
  `dehydrate_local(path)`, `create_directory(path)`, `rename(old_path, new_name)`,
  `remove(path)`.

Note the asymmetry: `file_status` lives on `MountsHandle` and takes the mount id
explicitly, while `VfsHandle` already carries its own. `hydrate_local` **commits
bytes the caller already fetched** — it is not a download; the network path is
`FolderSyncRunner::hydrate_file_from_dav` (`sync/runner.rs:772`) and the download
queue.

`MountsHandle::sync_mount(id)` is **local-only today**: the worker branch for
`OrchestratorCommand::SyncMount` (`worker.rs:376-437`) constructs a
`FolderSyncRunner` and calls `run_local_reconciliation()`
(`crates/core/nextcloud-sdk-orchestrator/src/sync/runner.rs:114`), which reconciles
the local directory against the journal with an *empty* remote set
(`runner.rs:140`, `reconcile(&local_entries, &[], &journal)`). No PROPFIND, no
download, no upload happens on that path, and the returned
`SyncCycleCompleted { items_processed }` counts planned local actions. A caller
that wants a real two-way cycle drives `FolderSyncRunner::execute_sync_cycle`
itself with its own `DavClient`, as the `discover_remote` +
`execute_sync_cycle` signature pair is public for exactly that reason. Do not
document `sync_mount()` as a network sync.

### 3.3 Mounts and modes

`MountDescriptor` (`mount/descriptor.rs`) connects one remote path to one local
target:

```rust
pub struct MountDescriptor {
    pub id: MountId,
    pub account_id: String,
    pub remote_path: String,   // "/", "/Photos", ...
    pub local_target: PathBuf,
    pub mode: MountMode,
    pub display_name: Option<String>,
    pub enabled: bool,
}
```

`MountDescriptor::new(account_id, remote_path, local_target, mode)` normalizes
`remote_path` to a leading `/` and assigns a fresh `MountId`; `with_display_name`
and `with_id` are the builder refinements, and `is_root()` / `relative_path(abs)`
are the queries a UI needs for breadcrumbs.

`MountMode` (`mount/mode.rs`) has three variants:

| Mode | Behaviour |
| --- | --- |
| `VirtualVfs` (default) | Placeholder tree in the cache; payloads arrive only on hydration |
| `BidirectionalSync` | Mirrors local target against the remote path; local edits flow back |
| `Fuse` | Presents the hierarchy as a real filesystem through `nextcloud-sdk-fuse` |

**`mode` is carried, not acted on.** Inside `nextcloud-sdk-orchestrator` the field
is only ever stored and echoed back: `mode` appears in `mount/descriptor.rs:24`
(the field), `:40`/`:54` (the constructor), and `mount/mod.rs:6`/`:14` (the module
and re-export). No branch in `worker.rs` or the mount manager reads it, and
`MountMode::Fuse` is constructed nowhere in the workspace. FUSE mounting is a
separate, explicit call — `mount_virtual_fs(FuseDriver, mountpoint)` — made by the
caller with its own `FuseDriver`. Treat the mode as a label the consumer persists
and interprets.

Multiple mounts per account are normal and expected — a root mount plus one per
synced subdirectory. `MountState`/`MountStatus` (`mount/status.rs`) report the
runtime state (running, paused, syncing, errored), and `SyncFileStatus` reports
per-file state: `Synced`, `OnlineOnly`, `Syncing`, `SyncingDown`, `SyncingUp`,
`Conflict`, `Ignored`, `Error(String)`.

### 3.4 Sub-systems

| Sub-system | Module | Responsibility |
| --- | --- | --- |
| Mount manager | `mount/manager.rs`, `mount/storage.rs` | Descriptor registry, per-mount state, `MountStorageUsage` |
| VFS manager | `vfs/manager.rs` | Owns each mount's `VirtualTree`, populates from DAV resources, persists it |
| Folder sync runner | `sync/runner.rs` | `FolderSyncRunner::new(local_dir, remote_dir, journal_path)`; `discover_remote(dav_client, user, remote_root) -> Vec<DiscoveredRemoteFile>` (recursive PROPFIND); `plan_reconciliation(&[DiscoveredRemoteFile], is_vfs) -> (Vec<SyncAction>, SyncJournal)`; `execute_sync_cycle(dav_client, user, is_vfs, is_cancelled, on_progress) -> FolderSyncOutcome`; `run_local_reconciliation()` (journal vs local dir, no network); plus the single-resource DAV helpers `hydrate_file_from_dav`, `upload_file_to_dav`, `create_remote_directory`, `move_remote_resource`, `delete_remote_resource` |
| Watcher | `sync/watcher.rs` | Local filesystem notifications (`notify`) feeding the runner |
| Download queue | `download/mod.rs` | Prioritized hydration/download scheduling, `DownloadProgress` events |
| Progress/status | `mount/progress.rs`, `mount/status.rs` | `MountProgressSnapshot`, `SyncFileStatus`, storage usage |

### 3.5 Events

`OrchestratorEvent` (`event.rs`) is the app's only source of runtime truth:

```text
MountAdded(MountDescriptor)              MountRemoved(MountId)
MountStatusChanged { id, status }        FileStatusChanged { mount_id, path, status }
FileHydrated { mount_id, remote_path, local_path }
FileDehydrated { mount_id, remote_path }
SyncCycleStarted { mount_id }            SyncCycleCompleted { mount_id, items_processed }
VfsTreeChanged { account_id, path }      AccountChanged { active_account_id }
GlobalSyncPaused                         GlobalSyncResumed
DownloadProgress(DownloadProgress)       MountProgress(MountProgressSnapshot)
```

A UI subscribes once, translates these into widgets, and never polls.

---

## 4. VFS and hydration model

`nextcloud-sdk-vfs` defines the data a file browser renders, independent of any UI
toolkit.

| Type | Module | Purpose |
| --- | --- | --- |
| `VirtualTree` | `tree.rs` | The mount's hierarchy; owns nodes, thumbnails, navigation |
| `VirtualNode` | `node.rs` | One entry: name, metadata, hydration state, permissions, file id |
| `NodeMetadata` | `metadata.rs` | Size, mime, mtime, etag, has-preview |
| `HydrationState` / `HydratedContent` | `hydration.rs` | `Placeholder` vs `Hydrated(HydratedContent)`, where `HydratedContent` is `Bytes(Vec<u8>)` or `Path(PathBuf)`; `as_bytes()`/`as_path()` and `From<Vec<u8>>`/`From<PathBuf>` provide access |
| `NextcloudPermissions` | `permissions.rs` | Decoded Nextcloud permission bitfield |
| `ResourceIconCategory` | `category.rs` | Icon classification from mime/extension |
| `DirectorySizeSummary`, `VirtualEntrySummary` | `dir_size.rs`, `entry.rs` | Aggregates for folder rows and properties |
| `FileSortField` / `FileSortOrder` / `FileSortCriteria` | `sort.rs` | Sorting vocabulary shared with the app |
| `NavigationHistory`, `BreadcrumbSegment` | `navigation.rs` | Back/forward and breadcrumbs |
| `ThumbnailCache` | `thumbnail.rs` | Multi-resolution thumbnail storage |

Hydration is a state transition, not a separate download path: the orchestrator
plans it, the queue executes it, `VfsHandle::hydrate_local` exposes it, and
`FileHydrated` announces it. Dehydration is the reverse and frees bytes while
keeping metadata.

---

## 5. FUSE presentation

`nextcloud-sdk-fuse` turns a `VirtualTree` into a real mount point, for
applications and tools that cannot speak WebDAV:

- `mount_virtual_fs(driver, mountpoint)` / `mount_virtual_fs_with_options(driver,
  mountpoint, options)` take the driver **first**, start a `fuser` session in the
  background and return a `BackgroundSession`. The plain variant mounts read-only
  (`FSName=Nextcloud`, `Subtype=nextcloud`, `RO`); on non-Linux targets it returns
  `io::ErrorKind::Unsupported`.
- `FuseDriver::new(tree)` for a read-only presentation;
  `FuseDriver::with_hydrator(tree, Arc<dyn AsyncHydrator>)` to fetch payloads on
  first read; `with_hydrator_and_root` to shift the visible root.
- `NoopHydrator` for offline/test drivers.
- Extended attributes expose cached thumbnails as `user.thumb.{width}x{height}`
  (`FuseDriver::getxattr` / `listxattr`, `driver.rs:360`/`:381`), so a file manager
  can render previews without its own download path. Sync state reaches file
  managers through the VFS entry metadata, not through a bespoke xattr.
- Thumbnail generators and indexers (`tumbler`, `tracker`, `localsearch`,
  `nautilus`, `dolphin`, …) are detected by process name and declined so a preview
  crawl cannot trigger a hydration storm.

---

## 6. Facade crate: `nexia-nextcloud-sdk`

`Client` is the per-account object an app keeps. It is built with `ClientBuilder`
and exposes four groups:

1. **Session**: `session_state()`, `is_authenticated()`, `current_user_id()`,
   `start_login()`, `poll_login()`, `logout()`, `clear_cache()`, `credentials()`,
   `server_status()`.
2. **Stateless API access**, in two families — those borrowing `&BasicAuth`
   (`credentials()`): `dav`, `shares`, `notifications`, `preview`, `trashbin`,
   `versions`, `uploads`, `deck`, `theming`; and those borrowing an
   `OcsClient` from `ocs()`: `users`, `activity`, `search`, `talk`. Plus
   `anonymous_theming()` (no argument — it probes `/status.php` and theming
   unauthenticated) and the associated `Client::new_vfs(root_name)`.
3. **Persistence**: `set_setting`, `get_setting`, `store_cached_theme`,
   `load_cached_theme`, `store_sync_mount`, `remove_sync_mount`,
   `list_sync_mounts`, `store_directory_sort`, `load_directory_sort`.
4. **Runtime knobs**: `set_upload_limit`, `set_download_limit`,
   `set_download_progress`, `set_upload_progress`, `subscribe_events`.

`ClientBuilder` covers what a real deployment needs: `server_url`,
`storage`/`storage_paths`, `store_key`, `key_store`, `keychain(app_id, account)`,
`initial_account`, `initial_credentials`, `retry_policy`, `timeout_config`,
`upload_limit`, `download_limit`, `on_download_progress`, `on_upload_progress`,
`branding`, then `async build() -> Result<Client, ClientError>`.

`ClientEvent` is the facade-level broadcast for configuration changes that are not
per-mount runtime state: `SyncMountAdded`, `SyncMountRemoved`, `SyncMountUpdated`,
`ThemeUpdated`. Runtime sync/hydration truth comes from `OrchestratorEvent`;
persisted configuration truth comes from `ClientEvent` — the two are
complementary, not duplicates.

`StoredSyncMount` (`storage/config_store.rs`) is the persisted mount record —
`id`, `account_id`, `remote_path`, `local_target`, `mode` (`"VirtualVfs"` /
`"BidirectionalSync"`), `display_name`, `enabled`, `created_at`, `updated_at` — so a
client can restore its mounts at launch and hand them to the orchestrator as
`MountDescriptor`s.

---

## 7. Application model (why the app stays thin)

A conforming application is: a window, a list of `Client`s, an
`OrchestratorHandle`, and a renderer for events. The SDK supplies the handles,
the event vocabulary and the state behind them; it supplies nothing that
presumes what the consumer is.

```text
launch
  ├─ first run: no mounts yet
  │     start_login() / poll_login()            (direct SDK request)
  │     users().capabilities()                  (direct SDK request)
  │     theming() / server_status()             (direct SDK request)
  │     MountsHandle::add_mount(MountDescriptor) once the user picks folders
  └─ steady state
        subscribe_events()  ──►  render
        VfsHandle::list_children / hydrate_local / dehydrate_local
        MountsHandle::sync_mount / pause_mount / progress / storage_usage
```

**First-run setup is the one place direct requests are expected**, because there
is no mount and nothing to orchestrate yet. Everything after that goes through
handles and events.

### Anti-patterns this architecture forbids

| Smell | Why it is wrong here |
| --- | --- |
| App re-implements the sync cycle's DAV half | Use `FolderSyncRunner::discover_remote` + `execute_sync_cycle`; only the *cadence* is the app's business, because the SDK ships no timer |
| App walks PROPFIND itself and builds a tree | `VfsHandle::populate_from_resources` / VFS manager own that |
| App keeps a mount registry beside `ConfigStore` | Two sources of truth; mounts drift and vanish |
| App re-implements ETag/mtime reconciliation | `nextcloud-sync::reconcile` + `SyncJournal` exist for this |
| App polls progress on a timer | `MountProgress`, `DownloadProgress`, `TransferProgress` events |
| App depends on `nextcloud-dav` / `nexia-nextcloud-core` directly | Wire types leak into the app; depend on the facade re-exports |

---

## 8. Concurrency and safety

1. **One stateful thread.** All mutable runtime state (mounts, trees, queues,
   journals) lives on the orchestrator thread. Handles are thin senders.
2. **No locks on the caller side.** App threads never block on the worker's
   internals; they await replies and consume a broadcast channel.
3. **Broadcast, not polling.** `broadcast::channel(512)` for
   `OrchestratorEvent` (`crates/core/nextcloud-sdk-orchestrator/src/worker.rs:29`),
   `broadcast::channel(128)` for the facade `ClientEvent` bus
   (`src/builder/client_builder.rs:268`). A lagging consumer sees
   `RecvError::Lagged` and should re-read state via a handle, never assume it can
   replay.
4. **Pure state machines stay pure.** `nextcloud-sdk-session` and
   `nextcloud-sdk-request` perform no I/O, hold no timers and log nothing, so they
   are unit-testable without a server.
5. **`#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`** across the
   workspace; credentials redact through `SecretString` and header redaction (see
   [`SECURITY_AND_STORAGE.md`](SECURITY_AND_STORAGE.md)).
