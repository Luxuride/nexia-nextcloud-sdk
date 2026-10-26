# nextcloud-rust-sdk

The full-featured Rust SDK for Nextcloud: a typed client for every API area plus
a managed runtime — mounts, synchronization, virtual-file hydration, thumbnails,
caching and event delivery.

**This crate is the engine, and it is generic.** It has no knowledge of any
particular consumer: no application is named here, none is special-cased, and
nothing in the API exists "for" a specific client. Any application that needs
Nextcloud's data asks the SDK to mount, sync, hydrate, dehydrate or classify a
file, and renders whatever the SDK tells it.

If a capability belongs to Nextcloud's protocols or to running transfers, the
fix belongs here, not in a downstream application.

---

## Two ways to use it

| Use | Start at | What you get |
| --- | --- | --- |
| **Managed runtime** | `Client` + `nextcloud-sdk-orchestrator` | A background actor thread owning mounts, sync cycles, VFS trees, a download queue, thumbnails and a broadcast event stream. The app sends commands and renders events. |
| **Stateless protocol access** | `DavClient`, `OcsClient`, `ShareClient`, … | Thin, borrow-only clients over a `Transport` + `AuthProvider` seam, for one-off requests and tests. |

A consumer uses both at different moments: the managed runtime for everything
steady-state, and the stateless clients for the direct requests that make up
first-run setup (before any mount exists) and for account/settings screens.

---

## Layout

The SDK is a Cargo workspace of small crates, one per concern, split into two
trees: `crates/core/` holds the Nextcloud-agnostic SDK layer (anything that
would serve a different API client just as well), and `crates/nextcloud/`
holds everything that knows Nextcloud's wire format.

### Generic SDK layer (`crates/core/`)

| Crate | Responsibility |
| --- | --- |
| `nextcloud-sdk-core` | HTTP methods and status codes, redacting headers, `ServerUrl`/`ApiPath`, the `Transport` and `AuthProvider` seams, the staged `Error` type |
| `nextcloud-sdk-session` | Session lifecycle state machine: login, grant, authenticated, re-authentication |
| `nextcloud-sdk-request` | Per-call lifecycle state machine, retry policy and failure classification |
| `nextcloud-sdk-storage` | Encrypted SQLCipher SQLite stores: credentials (`auth.db`), response cache (`cache.db`) and configuration (`config.db`); `StoreKey` handling and migrations |
| `nextcloud-sdk-keyring` | Multi-platform secure keyring abstraction (Linux FreeDesktop + Flatpak portal via `oo7`, macOS Keychain, Windows Credential Manager, Memory store) |
| `nextcloud-sdk-ratelimit` | Token-bucket bandwidth limiting, upload/download progress reporting |
| `nextcloud-sdk-vfs` | Virtual tree, node metadata, permissions, categories, sorting, navigation history, hydration states, multi-resolution thumbnail cache |
| `nextcloud-sdk-fuse` | `fuser`-based driver presenting a `VirtualTree` as a real filesystem, with on-demand hydration through an `AsyncHydrator` |
| `nextcloud-sdk-orchestrator` | The background runtime: `NextcloudOrchestrator` on a dedicated OS thread, `MountsHandle`/`VfsHandle` command surface, mount manager, folder-sync runner, filesystem watcher, download queue, `OrchestratorEvent` stream |

### Nextcloud layer (`crates/nextcloud/`)

| Crate | Responsibility |
| --- | --- |
| `nextcloud-auth` | App password, bearer token and anonymous credentials |
| `nextcloud-transport` | `reqwest`-backed implementation of `nextcloud_sdk_core::Transport` |
| `nextcloud-ocs` | OCS envelope (`meta`/`data`), mutations (POST, PUT, DELETE), and generic client |
| `nextcloud-dav` | WebDAV paths, file & collection CRUD operations (`get`, `put`, `mkcol`, `delete`, `move_`, `copy`), PROPFIND bodies and multistatus decoding |
| `nextcloud-users` | Server capabilities and user profile data |
| `nextcloud-shares` | OCS file sharing API, public links, share types, and permissions bitflags |
| `nextcloud-notifications` | Nextcloud Notifications v2 API and notification actions |
| `nextcloud-activity` | Nextcloud Activity v2 API streams and filter queries |
| `nextcloud-trashbin` | WebDAV trashbin client: listing, restoring, and permanently deleting items |
| `nextcloud-versions` | WebDAV file versioning client: revision history listing, downloading, and restoring revisions |
| `nextcloud-uploads` | WebDAV chunked upload streaming v2 client (`/remote.php/dav/uploads/{user}/...`) |
| `nextcloud-deck` | Nextcloud Deck Kanban boards, stacks, cards, and labels (`/index.php/apps/deck/api/v1.0/...`) |
| `nextcloud-talk` | Nextcloud Talk (Spreed) conversations, chat messages, and rooms (`/ocs/v2.php/apps/spreed/api/v4/room`, `/ocs/v2.php/apps/spreed/api/v1/chat`) |
| `nextcloud-preview` | Nextcloud thumbnail and preview generation client (`/index.php/core/preview.png`) plus the FreeDesktop thumbnail protocol |
| `nextcloud-theming` | Theming and custom branding (`/ocs/v2.php/apps/theming/api/v1/theme`), unauthenticated `/status.php` probing, `CustomBranding` overrides merged into `EffectiveBranding` |
| `nextcloud-search` | Unified Search providers and queries (`/ocs/v2.php/search/providers`) |
| `nextcloud-sync` | Bidirectional sync engine internals: 3-way `reconcile`, `SyncAction`, `SyncJournal`, local scanner, remote item metadata, download queue |
| `nextcloud-goa` | Direct GNOME Online Accounts (`org.gnome.OnlineAccounts`) D-Bus client via `zbus` |
| `nextcloud-kaccounts` | KDE Plasma Online Accounts (KAccounts / Accounts-SSO) reader for Nextcloud system accounts |
| `nextcloud-client-config` | `nextcloud.cfg` profile discovery and parser across Linux, macOS, and Windows, for adopt-on-first-run and migration flows |

### Facade (`src/`)

The root `nextcloud-rust-sdk` package re-exports the workspace vocabulary, so a
single dependency is enough, and adds the app-facing pieces:

| Module | Contents |
| --- | --- |
| `builder` | `ClientBuilder` — server URL, storage paths, keyring, initial account, retry policy, timeouts, bandwidth limits, progress callbacks |
| `client` | `Client`: session lifecycle (`start_login`, `poll_login`, `logout`), stateless client factories (`dav`, `ocs`, `users`, `shares`, `notifications`, `activity`, `search`, `preview`, `trashbin`, `versions`, `uploads`, `deck`, `talk`, `theming`), `new_vfs`, settings, directory sort order, and sync-mount persistence |
| `event` | `ClientEvent` broadcast: mount added/removed/updated, theme updated |
| `sso` | `DesktopSso` discovery coordinator aggregating GNOME, KDE and `nextcloud.cfg` sources |
| `storage` | `ConfigStore`, `SessionStore`, `CacheStore`, `StoragePaths`, `StoreKey`, `StoredSyncMount`, `CachedTheme` |
| `worker` | `Client` actor plumbing (commands, replies) |

---

## The managed runtime in one page

```text
                 +-------------------------------------------------+
  app thread     |  Client            MountsHandle      VfsHandle  |
  (UI/daemon)    |  subscribe_events()  add_mount()     hydrate_*  |
                 +-----------+----------------+---------------+----+
                             | commands (mpsc) |               |
                             v                v               v
                 +-------------------------------------------------+
  dedicated      |  NextcloudOrchestrator (own OS thread + Tokio)  |
  worker thread  |  mount manager | folder sync runner | download  |
                 |  filesystem watcher | VFS manager | cache       |
                 +---------------------------+---------------------+
                             | events (broadcast)
                             v
                 app renders: mount status, per-file sync state,
                 hydration progress, sync cycle outcomes
```

* `NextcloudOrchestrator::start(OrchestratorConfig)` spawns the worker thread and
  its own multi-threaded Tokio runtime (never the caller's runtime).
* `OrchestratorConfig { base_cache_dir, mounts, sync_interval_seconds, worker_threads }`
  — the cache dir holds the persisted tree, journals and thumbnails.
* `MountDescriptor { id, account_id, remote_path, local_target, mode, display_name, enabled }`
  with `MountMode::{ VirtualVfs, BidirectionalSync, Fuse }` — carried on the
  descriptor and persisted, but never branched on by the orchestrator; FUSE
  exposure is the explicit `mount_virtual_fs(FuseDriver, mountpoint)` call. One
  account can carry many mounts, including several subdirectory mounts of the
  same account.
* `MountsHandle` — `add_mount`, `remove_mount`, `list_mounts`, `get_mount`,
  `sync_mount` (local reconciliation against the journal), `pause_mount`/`resume_mount`,
  `storage_usage`, `clear_cache`, `clear_thumbnail_cache`, `progress`,
  `file_status`/`set_file_status`. The network cycle is
  `FolderSyncRunner::{discover_remote, execute_sync_cycle}`.
* `VfsHandle` (per mount) — `list_children`, `snapshot`, `entry`,
  `populate_from_resources`, `is_directory_populated`, `unpopulated_directories`,
  `hydrate_local`, `dehydrate_local`, `create_directory`, `rename`, `remove`.
* `OrchestratorHandle` — `mounts()`, `vfs(mount_id)`, `subscribe_events()`,
  `pause_all_sync`/`resume_all_sync`, `is_sync_paused`, `shutdown`.
* `OrchestratorEvent` — `MountAdded`, `MountRemoved`, `MountStatusChanged`,
  `FileStatusChanged`, `FileHydrated`, `FileDehydrated`, `SyncCycleStarted`,
  `SyncCycleCompleted`, `VfsTreeChanged`, `AccountChanged`, `GlobalSyncPaused`,
  `GlobalSyncResumed`, `DownloadProgress`, `MountProgress`.
* `SyncFileStatus::{ Synced, OnlineOnly, Syncing, SyncingDown, SyncingUp, Conflict, Ignored, Error }`
  is the vocabulary a file browser should render; the SDK classifies, the app paints.

The application never walks a directory tree to build state, never re-implements
hydration, and never re-derives per-file sync status. It holds handles,
subscribes, and renders.

One thing the app *does* own: the **cadence**. The SDK ships no timer —
`OrchestratorConfig::sync_interval_seconds` is stored but no worker loop reads it
today — so whichever component decides "sync now" (a `tokio::time::interval`, a
user action, a filesystem watcher) drives
`FolderSyncRunner::{discover_remote, execute_sync_cycle}` itself.

### What an app should never do

* Implement its own PROPFIND crawl, reconciliation, journal or retry loop — that
  is `nextcloud-sync` + `nextcloud-sdk-orchestrator`.
* Keep a parallel mount/account registry that disagrees with `ConfigStore` and
  `MountsHandle`.
* Poll for progress with its own timers — subscribe to `OrchestratorEvent` and
  `ClientEvent`, and read `MountsHandle::progress`.
* Assume `MountsHandle::sync_mount()` talks to the network. It reconciles the
  local directory against the journal only.
* Depend on the internal crates and their wire types; depend on
  `nextcloud-rust-sdk` and use the re-exports.

---

## Scope of this crate

What belongs in the SDK:

* Nextcloud wire formats, typed models, envelopes, retry classification, rate limiting.
* Session and credential handling, encrypted storage, keyring integration.
* The runtime that owns mounts, sync cycles, the VFS, hydration and the download queue.
* Event vocabulary rich enough that a consumer can render without re-deriving state.

What does not belong:

* Knowledge of a particular application, its UI toolkit, its window layout or its
  configuration file format.
* Consumers' naming, branding or product decisions.
* Workarounds for a single downstream integration.

---

## Development

Contributions from humans and LLM / AI-assisted contributors are both welcome.
Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for quality gates, code rules and the
required LLM PR template.

Consumers are not part of this repository's design surface: a change is reviewed
against the Nextcloud protocols and the SDK's own contracts, not against what one
particular application happens to need this month.

Build, test, format and lint inside the devcontainer:

```sh
cargo build
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Further reading:

* [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — layers, actors, data flow
* [`docs/GETTING_STARTED.md`](docs/GETTING_STARTED.md) — from zero to a synced mount
* [`docs/API_REFERENCE.md`](docs/API_REFERENCE.md) — endpoint → crate → method matrix
* [`docs/SECURITY_AND_STORAGE.md`](docs/SECURITY_AND_STORAGE.md) — encryption, keyrings, redaction

## License

Licensed under the GNU General Public License, version 3 or later
(`GPL-3.0-or-later`). See [`LICENSE`](LICENSE).
