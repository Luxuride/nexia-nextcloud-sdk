# Getting Started with the Nextcloud Rust SDK

This guide takes an integrator from an empty `Cargo.toml` to a client with a live
mount and an event stream — the shape a real application actually has.

It is written for the intended consumer shape: a **thin application** (GUI,
daemon, CLI or service) that delegates mounting, synchronization, hydration and
thumbnailing to the SDK and only renders what comes back.

The SDK has no knowledge of any specific consumer — nothing below is
client-specific, and no client is named in the API.

Two integration modes exist and a real app uses both:

- **Managed runtime** — `NextcloudOrchestrator` owns mounts, sync cycles, the VFS
  and the download queue. Use this for everything steady-state.
- **Direct requests** — stateless clients (`DavClient`, `UserClient`, …) for the
  one-off calls that make up first-run setup, account screens and settings.

---

## 1. Adding dependencies

```toml
[dependencies]
nextcloud-rust-sdk = "0.1.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

The facade re-exports the runtime types, so one dependency covers session
handling, stateless clients **and** the orchestrator:

```rust
use nextcloud_rust_sdk::{
    Client, ClientBuilder, ClientEvent, MountDescriptor, MountMode, MountsHandle,
    NextcloudOrchestrator, OrchestratorConfig, OrchestratorEvent, OrchestratorHandle,
    SyncFileStatus, VfsHandle,
};
```

Depend on individual workspace crates only when you are building tooling that
must not carry the runtime (for example a CLI that only reads capabilities):

```toml
[dependencies]
nextcloud-sdk-core = "0.1.3"
nextcloud-auth = "0.1.3"
nextcloud-transport = "0.1.3"
nextcloud-dav = "0.1.3"
```

---

## 2. First run: authenticate with direct requests

Before any mount exists there is nothing to orchestrate, so setup talks to the
server directly. Login Flow v2 keeps the user's real password — and any 2FA —
inside the browser; the app receives an app password.

```rust
use nextcloud_rust_sdk::{Client, ClientBuilder, ServerUrl, SessionState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_url = ServerUrl::parse("https://cloud.example.org")?;

    let client = ClientBuilder::new()
        .server_url(server_url)
        .keychain("org.example.NextcloudApp", "default_account")
        .build()
        .await?;

    // 1. Ask the SDK to start the flow; it returns the URL the browser opens.
    let flow = client.start_login().await?;
    println!("Open this URL to authorize: {}", flow.login_url());

    // 2. Poll until the user grants access. The SDK owns the poll schedule.
    loop {
        match client.poll_login().await? {
            SessionState::Authenticated(_) => break,
            SessionState::ReauthRequired(_) => {
                return Err("authorization was rejected".into());
            }
            _ => tokio::time::sleep(std::time::Duration::from_secs(3)).await,
        }
    }

    println!("signed in as {:?}", client.current_user_id());
    Ok(())
}
```

After this, `client.is_authenticated()` is `true`, the app password is in the
encrypted credential store, and the `Client` is reusable across restarts by
rebuilding it with the same keyring identity.

### 2.1 Read what the server offers (still direct)

Capabilities decide what the UI may show later — chunked uploads, chunk sizes,
theming, trashbin, versions.

```rust
// `ocs()` borrows the current credentials; the API-specific clients borrow it.
let ocs = client.ocs()?;

let caps = client.users(&ocs).capabilities().await?;
if let Some(version) = caps.server_version() {
    println!("server version: {version}");
}
if let Some(files) = caps.files() {
    println!("chunked upload: {:?}", files.chunked_upload());
}

let me = client.users(&ocs).current_user().await?;
println!("display name: {}", me.display_name());

// Unauthenticated probes are safe before login.
let status = client.server_status().await?;
println!("server reachable: {}", status.installed);
let branding = client.anonymous_theming().get_theme().await?;
```

The call takes `&OcsClient`, not a credential: `dav`, `shares`, `notifications`,
`preview`, `trashbin`, `versions`, `uploads`, `deck` and `theming` take
`&BasicAuth` (from `client.credentials()`), while `users`, `activity`, `search`
and `talk` take the `OcsClient`.

### 2.2 Choose mount points

A mount is one remote path bound to one local target. A real client creates one
mount per folder the user selects — a root mount plus any number of
subdirectory mounts, all on the same account.

```rust
use nextcloud_rust_sdk::{MountDescriptor, MountMode};

let documents = MountDescriptor::new(
    "alice",                    // account_id
    "/Documents",               // remote path
    "/home/alice/Nextcloud/Documents",
    MountMode::VirtualVfs,      // metadata-only placeholders, hydrate on open
)
.with_display_name("Documents");

let photos = MountDescriptor::new(
    "alice",
    "/Photos",
    "/home/alice/Pictures/Nextcloud",
    MountMode::BidirectionalSync, // real files both ways
);
```

`MountDescriptor::new` normalizes `remote_path` to always start with `/`, assigns a
fresh `MountId`, and returns a descriptor with `enabled: true`. `with_display_name`,
`with_id` and `is_root()` are the other builder helpers.

`MountMode::Fuse` is the third variant. It is a **label**, not a behaviour
switch: the orchestrator stores it and never branches on it, so declaring
`MountMode::Fuse` does not create a filesystem. To get one you build a
`FuseDriver` and call the lower-level API yourself:

```rust
use nextcloud_rust_sdk::{mount_virtual_fs, FuseDriver};

let driver = FuseDriver::new(/* a VirtualTree snapshot */ client.new_vfs("Nextcloud"));
let _session = mount_virtual_fs(driver, "/home/alice/NextcloudVfs")?; // RAII: unmounts on drop
```

Persist them so the next launch restores them exactly. The persisted record is
`StoredSyncMount` (`id`, `account_id`, `remote_path`, `local_target`, `mode` as
`"VirtualVfs"` / `"BidirectionalSync"`, `display_name`, `enabled`, `created_at`,
`updated_at`):

```rust
use nextcloud_rust_sdk::StoredSyncMount;

client
    .store_sync_mount(StoredSyncMount {
        id: documents.id.to_string(),
        account_id: documents.account_id.clone(),
        remote_path: documents.remote_path.clone(),
        local_target: documents.local_target.display().to_string(),
        mode: "VirtualVfs".to_string(),
        display_name: documents.display_name.clone(),
        enabled: documents.enabled,
        created_at: 0,
        updated_at: 0,
    })
    .await?;

let stored = client.list_sync_mounts().await?;
```

---

## 3. Steady state: hand control to the orchestrator

This is the part that makes the application thin. The orchestrator owns its own
OS thread and its own Tokio runtime; the app only sends commands and reads
events.

```rust
use nextcloud_rust_sdk::{NextcloudOrchestrator, OrchestratorConfig, OrchestratorEvent};

let config = OrchestratorConfig::new("/home/alice/.cache/nextcloud")
    .with_mount(documents)
    .with_mount(photos);
// sync_interval_seconds defaults to 60 and worker_threads to 2, but note that
// nothing reads the interval yet: add_mount / sync_mount do not schedule
// periodic cycles, so drive the cadence from your own timer or watcher.

let orchestrator = NextcloudOrchestrator::start(config)?;
let handle = orchestrator.handle();

// A UI subscribes once and renders from here on.
let mut events = handle.subscribe_events();
while let Ok(event) = events.recv().await {
    match event {
        OrchestratorEvent::MountStatusChanged { id, status } => {
            println!("mount {id}: {status:?}");
        }
        OrchestratorEvent::FileStatusChanged { mount_id, path, status } => {
            // status is a SyncFileStatus — paint the badge, do not recompute it.
            println!("{mount_id}:{path} -> {status:?}");
        }
        OrchestratorEvent::FileHydrated { remote_path, local_path, .. } => {
            println!("{remote_path} available at {}", local_path.display());
        }
        OrchestratorEvent::SyncCycleCompleted { mount_id, items_processed } => {
            println!("{mount_id} synced {items_processed} items");
        }
        OrchestratorEvent::DownloadProgress(_) | OrchestratorEvent::MountProgress(_) => {}
        _ => {}
    }
}
```

`orchestrator.shutdown()` (or dropping it) sends the shutdown command and joins
the worker thread.

### 3.1 Mount commands

```rust
let mounts = handle.mounts();

// Add or remove mounts at runtime.
let new_id = mounts.add_mount(photos).await?;
let removed = mounts.remove_mount(new_id.clone()).await?;

// Enumerate mounts with their live state.
for (descriptor, state) in mounts.list_mounts().await? {
    println!("{:?} {:?} -> {}", descriptor.mode, state.status, descriptor.local_target.display());
}

// Run a cycle, or hold everything still.
// NOTE: `sync_mount` currently performs LOCAL reconciliation only (journal vs
// local directory, no DAV round-trip). For a real two-way cycle use
// `FolderSyncRunner::{discover_remote, execute_sync_cycle}` with your own
// `DavClient` — see ARCHITECTURE.md §3.2.
mounts.sync_mount(new_id).await?;
mounts.pause_mount(new_id).await?;
mounts.resume_mount(new_id).await?;

handle.pause_all_sync().await?;
handle.resume_all_sync().await?;
```

### 3.2 File state and storage

```rust
use nextcloud_rust_sdk::SyncFileStatus;

// Per-file state inside a mount. `file_status` takes the mount id explicitly;
// `status_for_local_path` resolves an absolute path across all mounts.
match mounts.file_status(new_id.clone(), "2024/trip.jpg").await? {
    SyncFileStatus::OnlineOnly => { /* show a cloud badge */ }
    SyncFileStatus::Conflict => { /* surface a conflict */ }
    SyncFileStatus::Error(msg) => { /* show the message, do not retry manually */ }
    _ => {}
}

let usage = mounts.storage_usage(new_id.clone()).await?;
println!(
    "{} bytes cached in {} files, {} thumbnail bytes",
    usage.local_bytes, usage.hydrated_files_count, usage.shared_thumbnail_bytes
);

let progress = mounts.progress(new_id.clone()).await?;
println!(
    "{}/{} items, {} of {} bytes, status {:?}",
    progress.completed_items, progress.total_items,
    progress.bytes_transferred, progress.bytes_total, progress.status
);

// Reclaim space without touching the server.
let freed = mounts.clear_cache(new_id.clone(), true).await?;
let freed_thumbs = mounts.clear_thumbnail_cache().await?;
```

### 3.3 The VFS: the file browser's data source

`VfsHandle` is bound to one mount and is the only thing a file list needs.

```rust
let vfs = handle.vfs(new_id.clone());

// Direct children of a directory (paths are relative to the mount root).
// A `VirtualEntrySummary` carries `path`, `name`, `is_directory`, `size`, `etag`,
// `modified_at`, `permissions`, `mime_type`, `file_id`, `icon_category`,
// `is_hydrated` and `cached_path` — enough to paint a row without a second call.
for child in vfs.list_children("/").await? {
    println!(
        "{} {} bytes hydrated={} icon={:?}",
        child.name, child.size, child.is_hydrated, child.icon_category
    );
}

// A whole tree snapshot when the UI needs the full structure.
let tree = vfs.snapshot().await?;

// A single entry, and which directories still need a server round-trip.
let entry = vfs.entry("2024/trip.jpg").await?;
let pending = vfs.unpopulated_directories().await?;

// `hydrate_local` commits bytes the caller already fetched; it does not download.
let bytes = client.dav(&auth).get("Photos/2024/trip.jpg").await?;
let local_path = vfs.hydrate_local("2024/trip.jpg", bytes).await?;
vfs.dehydrate_local("2024/trip.jpg").await?;

// Mutations go through the handle so tree, cache and events stay consistent.
vfs.create_directory("2024").await?;
vfs.rename("2024/trip.jpg", "trip-final.jpg").await?;
vfs.remove("2024/trip-final.jpg").await?;
```

`populate_from_resources(dav_prefix, resources)` feeds a DAV listing into the tree
when a directory is expanded; the orchestrator decides when to call it. An
application should almost never need it directly.

Never build a second tree in the UI layer: hydrate/dehydrate/rename through the
handle, then react to the events.

### 3.4 FUSE presentation

When an application needs a real path rather than an API:

```rust
use nextcloud_rust_sdk::{mount_virtual_fs, FuseDriver, VirtualTree};
use std::sync::{Arc, RwLock};

let tree: Arc<RwLock<VirtualTree>> = Arc::new(RwLock::new(vfs.snapshot().await?));
let driver = FuseDriver::new(tree);
let session = mount_virtual_fs(driver, "/run/user/1000/nextcloud")?;
// session.unmount() when done
```

`mount_virtual_fs` mounts read-only (`FSName=Nextcloud`, `Subtype=nextcloud`,
`RO`) and returns a `BackgroundSession`; use `mount_virtual_fs_with_options` for
read-write or custom options. On non-Linux targets it returns
`io::ErrorKind::Unsupported`.

`FuseDriver::with_hydrator(tree, hydrator)` fetches payloads on first read, so the
mount behaves like a lazy local filesystem; `NoopHydrator` gives a read-only view.

---

## 4. Direct requests after setup

Any screen that is not mount state — shares, notifications, activity, trash,
versions, search, Deck, Talk — is a direct call on a client handle. These are
stateless: they borrow the transport and return owned data.

```rust
let auth = client.credentials().ok_or("not signed in")?;
let ocs = client.ocs()?;

// WebDAV file operations — every path is a `DavPath`, not a bare &str.
use nextcloud_rust_sdk::{DavPath, Depth, Overwrite, PropfindBody};
// Preview, activity and share helpers used further down.
use nextcloud_rust_sdk::{ActivityFilter, CreateShare, PreviewMode, PreviewOptions};

let user = client.current_user_id().ok_or("not signed in")?;
let documents = DavPath::new(&user, "Documents")?;
let dav = client.dav(&auth);

let resources = dav.propfind(&documents, Depth::One, &PropfindBody::all()).await?;
let bytes = dav.get(&DavPath::new(&user, "Documents/report.pdf")?).await?;
dav.put(
    &DavPath::new(&user, "Documents/notes.txt")?,
    b"hello".to_vec(),
    Some("text/plain"),
)
.await?;
dav.mkcol(&DavPath::new(&user, "Documents/archive")?).await?;
dav.move_(
    &DavPath::new(&user, "Documents/notes.txt")?,
    &DavPath::new(&user, "Documents/archive/notes.txt")?,
    Overwrite::Allow,
)
.await?;

// Shares
let shares = client.shares(&auth).list(Some("Documents")).await?;
let link = client.shares(&auth).create(&CreateShare::public_link("Documents/report.pdf")).await?;

// Notifications and activity (activity borrows the OcsClient)
for n in client.notifications(&auth).list().await? {
    println!("{}: {}", n.app, n.subject);
}
for a in client.activity(&ocs).list(&ActivityFilter::default()).await? {
    println!("{a:?}");
}

// Search (per provider), previews (by file id or by path), trash, versions
for provider in client.search(&ocs).list_providers().await? {
    let page = client
        .search(&ocs)
        .search(&provider.id, "quarterly report")
        .await?;
    println!("{:?}", page.entries);
}
let options = PreviewOptions::new()
    .dimensions(256, 256)
    .mode(PreviewMode::Cover);
let thumb = client.preview(&auth).get_by_file_id(file_id, &options).await?;
let thumb_by_path = client.preview(&auth).get_by_path("Documents/report.pdf", &options).await?;
// Trashbin and versions are addressed per user id
let trashed = client.trashbin(&auth).list(&user).await?;
let history = client.versions(&auth).list(&user, file_id).await?;
```

The full endpoint → crate → method matrix is in
[`API_REFERENCE.md`](API_REFERENCE.md).

### 4.1 Bandwidth and progress

```rust
use std::sync::Arc;

use nextcloud_rust_sdk::{BandwidthLimit, TransferProgress};

client.set_download_limit(BandwidthLimit::from_mib(50));
client.set_upload_limit(BandwidthLimit::from_mib(10));

// `ProgressCallback` is a type alias for
// `Arc<dyn Fn(TransferProgress) + Send + Sync>` — construct it with `Arc::new`.
client.set_download_progress(Some(Arc::new(|p: TransferProgress| {
    match p.percentage() {
        Some(pct) => println!("{pct:>6.1}%"),
        None => println!("{} bytes", p.transferred_bytes()),
    }
})));
```

`BandwidthLimit::{unlimited(), bytes_per_second(n), from_kib(n), from_mib(n)}`;
`0` collapses to `Unlimited`. Progress callbacks are for a single transfer. Aggregate, mount-level progress is
`MountProgressSnapshot` from the orchestrator — the two are not interchangeable.

---

## 5. Event streams: what to subscribe to

| Stream | Type | Carries |
| --- | --- | --- |
| Runtime sync truth | `OrchestratorHandle::subscribe_events()` | mount status, per-file state, hydration, sync cycles, download/mount progress |
| Persisted config changes | `Client::subscribe_events()` | `ClientEvent`: `SyncMountAdded`, `SyncMountRemoved`, `SyncMountUpdated`, `ThemeUpdated` |
| App-level view state | your own bus | selected view, filters, dialogs — nothing the SDK knows about |

Use the first two; never invent a polling loop for either. Both are
tokio `broadcast` receivers: a lagging receiver gets `RecvError::Lagged(n)` and
must re-read state through a handle, because the skipped events are gone, not
queued.

---

## 6. Storage and keyring

The SDK owns persistence. Three encrypted SQLCipher SQLite stores live under
`StoragePaths`: `auth.db` (accounts, app passwords, login-flow state), `cache.db`
(capabilities and WebDAV listings) and `config.db` (settings, themes, sync
mounts). Each is opened with `EncryptedDatabase::open_credential_store(path, &key)`,
which runs `PRAGMA key` and a fail-fast `SELECT count(*) FROM sqlite_master` so a
wrong key fails as `StorageError::KeyRejected` before any query runs.

The 256-bit `StoreKey` is generated from OS entropy and persisted through the
platform keyring (`oo7` Secret Service / Flatpak portal on Linux, Keychain on
macOS, Credential Manager on Windows). See
[`SECURITY_AND_STORAGE.md`](SECURITY_AND_STORAGE.md) for the full model.

```rust
use nextcloud_rust_sdk::{StoragePaths, StoreKey, KeychainKeyStore, KeyStore};

let paths = StoragePaths::default_xdg()?;
paths.ensure_directories()?;

let keystore = KeychainKeyStore::new("org.example.NextcloudApp", "default_account");
let key: StoreKey = keystore.load_or_create().await?;
```

---

## 7. Checking your integration

A thin integration satisfies all of these. Use it as a review checklist:

- [ ] Reconciliation is not hand-written: `nextcloud_sync::reconcile` /
      `SyncJournal` decide what changed. A timer that merely *calls*
      `FolderSyncRunner::execute_sync_cycle` is expected — the SDK ships no
      cadence.
- [ ] No PROPFIND crawl in the UI crate; the VFS handle supplies listings.
- [ ] No local journal, etag cache or mount registry beside `ConfigStore`.
- [ ] Progress comes from `OrchestratorEvent`, not from polling commands.
- [ ] First-run setup uses direct requests only until the first mount is added.
- [ ] Only `nextcloud-rust-sdk` is a dependency, not the internal crates.

---

## 8. Development gates

```sh
cargo build
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

All must pass inside the devcontainer before a change is done. Network behaviour
is tested against a mock transport, never a live server or real credentials.
