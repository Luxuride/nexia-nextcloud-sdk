# Changelog

All notable changes to the Nextcloud Rust SDK workspace.

## Unreleased

### Added

- `nexia-nextcloud-core::url::{percent_encode, percent_encode_path}` — the single
  RFC 3986 encoder for the workspace. Query values, form values and path segments
  previously had three separate implementations that had already drifted.
- `UploadSession::with_chunk_size` and `UploadSession::chunk_size`: the session now
  remembers the chunk size it was started with, which is what makes completeness
  checkable at all.
- `UploadSession::is_supported_chunk_size` for the range the server accepts.
- `nexia-nextcloud-sdk-keyring::find_first_secret`, the ordered service × account
  lookup that three crates had copied.
- `KAccountsError::UnsupportedSchema`, naming the tables that were found instead of
  letting a `no such table` error stand in for "layout not recognised".
- `nexia-nextcloud-sync`: `reconcile_local_changes`, an upload-only planner for a
  local rescan, plus `is_ignored_name` as the single definition of the names a
  scan skips.
- `UrlError::InvalidPort` for a port outside `u16`. A malformed port used to be
  reported as `MissingHost`.

### Fixed

- `nexia-nextcloud-uploads`: `finish` refuses to assemble a session whose expected
  chunks are missing, and `upload_chunk` rejects a chunk whose length or index
  does not match the session. Assembly moves whatever the server holds into the
  destination, so a silently truncated upload used to be reported as success and
  could no longer be completed.
- `nexia-nextcloud-uploads`: the upload id comes from the OS random source. A
  `<millis>-<counter>` id was guessable and collided across processes, and the id
  names a collection anyone who guesses it can write into.
- `nexia-nextcloud-uploads`: an empty upload is complete once it has no chunks,
  instead of expecting chunk 1 forever.
- `nexia-nextcloud-sdk-keyring`: `SecretString` zeroes through `zeroize`, so the
  wipe survives optimisation, and `into_inner` no longer leaves a second plaintext
  copy behind.
- `nexia-nextcloud-sdk-storage`: `StoreKey` is zeroed on drop, and its hex and
  pragma forms are wrapped in `Zeroizing` — a plain `String` holding the database
  key was freed unzeroized.
- `nexia-nextcloud-sdk-vfs`: `read_hydrated_bytes` reports an unreadable cache file
  as `VfsError::Io` instead of `Ok(None)`, which was indistinguishable from a
  placeholder.
- `nexia-nextcloud-sdk-vfs`: inserting a directory no longer scans every known
  node to adopt orphans, and `remove`, `rename` and `move_node` walk a key range
  for their subtree instead of filtering the whole map. The node map is a
  `BTreeMap`, so a subtree is one ordered range.
- `nexia-nextcloud-kaccounts`: accounts are read from the `Settings`/`Services`
  tables libaccounts-glib creates, with the flat `AccountSettings`/
  `AccountServices` layout still supported. The layout is detected from the
  database, so a real KDE Plasma `accounts.db` no longer fails with
  `no such table`.
- `nexia-nextcloud-goa`: a GNOME Online Account that cannot be read is skipped with
  a warning instead of aborting the whole listing.
- `nexia-nextcloud-client-config`, `nexia-nextcloud-kaccounts`, `nexia-nextcloud-sdk`
  (SSO): a bare server URL and a bare KAccounts id are no longer keyring
  candidates. Neither names a user, so on a server with several accounts one
  account could pick up another account's token.
- `nexia-nextcloud-sdk-session`: `PollFailed` and `Expired` now end a pending
  Login Flow with `Command::StopPolling`. Both events existed and were documented
  but no transition accepted them, so a poll that could never succeed left the
  machine pending forever.
- `nexia-nextcloud-sdk-session`: `PollToken`'s `Debug` redacts the token and the
  type no longer implements `Display`. Both previously printed the raw value, and
  every owning state derives `Debug`.
- `nexia-nextcloud-core`: `Request`'s `Debug` prints the body length instead of
  the bytes, so a login or password body cannot reach a log.
- `nexia-nextcloud-core`: `ServerUrl` rejects an empty host (`https://:8080`).
- `nexia-nextcloud-core`: `StatusError` truncation decodes a bounded prefix of the
  body instead of the whole thing, so a multi-megabyte error page no longer costs
  a full lossy conversion plus a full character count.
- `nexia-nextcloud-ocs`: a payload that fails to decode is only tolerated when the
  document really is a well-formed OCS envelope. A garbled body previously became
  an empty success whenever the requested type deserialised from `null`.
- `nexia-nextcloud-notifications`, `nexia-nextcloud-shares`: an envelope with no
  readable verdict is a failure, not a success.
- `nexia-nextcloud-sync`: the journal is written to a temporary file, flushed and
  renamed, so a crash cannot leave a truncated journal that discards the whole
  sync baseline.
- `nexia-nextcloud-sync`: the download queue claims a task in one critical
  section, so two workers can no longer pick the same pending download, and a
  late progress or completion event no longer overwrites a cancelled or failed
  task. Reconciliation plans are deterministic (`BTreeSet`).
- `nexia-nextcloud-sdk-orchestrator`: `AddMount` reports the `MountManager` error
  instead of replying `Ok` with an id nothing can act on, and no `VfsManager` is
  registered for a rejected mount.
- `nexia-nextcloud-sdk-orchestrator`: the filesystem watcher exits when its
  channel closes instead of panicking in `tokio::select!` with every branch
  disabled.
- `nexia-nextcloud-sdk-orchestrator`: `hydrate_local`/`dehydrate_local` reject a
  relative path that is empty, absolute or carries a `.`/`..` segment, so a
  server-supplied name can no longer write outside the cache directory.
- `nexia-nextcloud-sdk-orchestrator`: `find_mount_for_path` picks the longest
  matching target, so a mount nested inside another is attributed correctly.
- `nexia-nextcloud-sdk-fuse`: a metadata date that overflows `SystemTime` no
  longer panics `getattr`; date arithmetic is checked.
- `nexia-nextcloud-preview`: `freedesktop_thumbnail_is_current` compares the
  stored `Thumb::MTime` with the file, so a changed file gets a fresh thumbnail,
  and a payload that cannot be decoded is no longer installed under a `.png`
  name.
- `nexia-nextcloud-dav`: `get_range` reduces a `200` answer to the requested
  window instead of returning the first bytes of the file as if they were the
  range, rejects an inverted range, and reconciles a `206` whose `Content-Range`
  starts later than requested.
- `nexia-nextcloud-dav`: a propstat status line without a reason phrase
  (`HTTP/1.1 200`) is accepted; matching on `" 200 "` dropped every property of
  such a response. A file whose name contains `collection` is no longer reported
  as a directory.
- `nexia-nextcloud-dav`/`nexia-nextcloud-uploads`: the clients print a constant
  `Debug`, so a credential source held by reference cannot be logged through the
  client.
- `nexia-nextcloud-uploads`: chunk boundaries saturate instead of overflowing,
  `chunk_boundaries` no longer reserves one entry per byte of the file for a tiny
  chunk size, and a chunk index is bounded by the five-digit file name.
- `nexia-nextcloud-versions`: a segment containing `%` is rejected, because the
  server decodes escapes and `%2F` would address a different resource.
- `nexia-nextcloud-search`: `provider_id` is percent-encoded before it is
  interpolated into the path.
- `nexia-nextcloud-sync`: a rescan plans uploads from the local side instead of
  reusing the full three-way plan, which scheduled a `DeleteLocal` for every
  journaled path. Mechanical noise (`~$…`, `.part`, `.crdownload`, `.DS_Store`) is
  skipped and no longer looks like a deletion, while dotfiles are scanned again.
- `nexia-nextcloud-sdk-fuse`: a parallel range fetch fails the span when a part
  does not have the requested length, instead of concatenating shifted bytes into
  the cache; the block cache is bounded per file and per mount, keeps the newest
  spans, and serves a read without cloning the span.
- `nexia-nextcloud-sdk-fuse`: a panic in the hydration block no longer panics the
  mount; the read fails with `EIO`.
- `nexia-nextcloud-sdk-vfs`: `extract_clean_dav_path` matches the DAV marker on
  the raw href and only decodes the extracted path, so a `%2F` in a name is no
  longer a separator and an empty prefix no longer turns the whole URL into a
  path.
- `nexia-nextcloud-sdk-storage`: the database directory is created `0700` and the
  file `0600`, and `PRAGMA temp_store = MEMORY` keeps sorts over cached rows from
  spilling plaintext into an unencrypted temp file.
- `nexia-nextcloud-kaccounts`: the accounts database is looked up in
  `libaccounts-glib` under the config directory first, where the daemon actually
  writes it.

### Changed

- Breaking: `ShareType` and `RoomType` gained an `Unknown(u8)` variant, `from_u8`
  returns `Self` instead of a `Result`, and `UnknownShareTypeError` /
  `UnknownRoomTypeError` are gone. A share type (2, 5, 9, 12 for Deck) or room
  type (4 for the changelog conversation) that the enum does not model made the
  whole listing fail; it is now preserved and flagged through `is_known`.
- Breaking: `DavPath` percent-encodes the user name and each path segment in the
  request target while `relative()` and `segments()` keep returning the decoded
  name. A name with a space, `%`, `?`, `#` or a non-ASCII letter used to be sent
  literally and either failed or addressed a different resource; `?` and `#` are
  now accepted as name characters instead of being rejected.
- Breaking: `UploadClient::start` and `start_with_id` take the chunk size, and
  `UploadClient::finish_verified` is gone because `finish` verifies now.
- Breaking: `VfsError` gained `Io { operation, path, source }` and no longer
  implements `Clone`, `PartialEq` or `Eq`, because it wraps `std::io::Error`.
- Breaking: `StoreKey::to_hex` and `StoreKey::pragma` return `Zeroizing<String>`.
- `ConfigStore::store_theme` returns the stored `CachedTheme` and
  `ConfigStore::store_sync_mount` returns the stored `StoredSyncMount`, so the
  emitted event and the persisted row carry the same `updated_at`. Both returned
  `()` and the event carried the caller's timestamp instead of the stored one.
- The Login Flow poll body percent-encodes the token.

## 0.2.2

### Fixed

- `nexia-nextcloud-sdk-fuse`: reads no longer hydrate a large file wholesale when
  only a small range was asked for. Hydration is bounded to the requested window,
  a large read is fetched in one request rather than one per window, readahead
  only happens when the reader is provably sequential, and a directory sweep no
  longer triggers speculative reads of files that were merely listed.
- `nexia-nextcloud-sdk-fuse`: a burst of concurrent reads no longer starves the
  shared worker pool, so metadata stays answerable while a mount is busy.
- `nexia-nextcloud-sdk-fuse`: preview passes decline instead of downloading the
  original, so a thumbnail request cannot pull a full-size body over the network.

### Changed

- Publishing runs in GitHub Actions authenticated by crates.io trusted
  publishing, so no `CARGO_REGISTRY_TOKEN` secret is stored. The workflow
  publishes in dependency order and waits out crates.io rate limits itself.
- `release-plz` is no longer used; a release is an explicit `version` bump plus a
  manual workflow run. `CONTRIBUTING.md` documents the flow.

## 0.2.1

### Changed

- Every crate in the workspace now carries the `nexia-nextcloud-` prefix. The 27
  crates that were still named `nextcloud-*` are `nexia-nextcloud-dav`,
  `nexia-nextcloud-ocs`, `nexia-nextcloud-sync` and so on, so no package on the
  registry reads as if it were published by Nextcloud GmbH. Import paths change
  accordingly (`nexia_nextcloud_dav::`, `nexia_nextcloud_ocs::`, ...).
- The 0.2.0 releases of the four crates that shipped before this rename
  (`nexia-nextcloud-core`, `nexia-nextcloud-search`, `nextcloud-ocs`,
  `nextcloud-activity`) are yanked; depend on 0.2.1 instead.

## 0.2.0

### Breaking

- The published crate is now `nexia-nextcloud-sdk` (was `nextcloud-rust-sdk`).
  Its two renamed dependencies are `nexia-nextcloud-core` (was
  `nextcloud-sdk-core`) and `nexia-nextcloud-search` (was `nextcloud-search`),
  so import paths become `nexia_nextcloud_sdk::`, `nexia_nextcloud_core::` and
  `nexia_nextcloud_search::`. No API was removed; only the crate names moved.
- Runtime identity strings move with the rename: the default data directory,
  the default keyring service and the transport `User-Agent` are all
  `nexia-nextcloud-sdk`. Credentials stored under the old service name are not
  found and must be re-authenticated.
- `nexia-nextcloud-preview`: the FreeDesktop `Software` PNG chunk is no longer
  hardcoded to a fixed product string. The generating program is caller
  knowledge under the Thumbnail Managing Standard, so it is now a parameter:
  `install_freedesktop_thumbnail[_with_mtime]()`, `create_freedesktop_png()` and
  `embed_freedesktop_metadata()` take `software: &str`;
  `embed_freedesktop_metadata_raw()` takes `Option<&str>` and omits the chunk
  when `None`. `install_freedesktop_fail_marker()` is unchanged on the outside
  and now writes no `Software` chunk, since a fail sentinel is not a generated
  thumbnail.

### Fixed

- Thumbnail-cache cleanup in consumers no longer has to pattern-match a
  hardcoded product name to recognise its own cache entries.

### Changed

- The `Nextcloud Desktop` string is no longer used as an example client name.
- The README states that this is an unofficial SDK and that "Nextcloud" names
  the APIs implemented, not an affiliation with Nextcloud GmbH.


## 0.1.2

### Added

- Zero-cost structured logging with `tracing` across all workspace crates and the facade client (`trace!`, `debug!`, `info!`, `warn!`, `error!`), with zero credential leakage.
- `nexia-nextcloud-theming`: theming metadata, custom client branding overrides, and unauthenticated server status endpoint (`/status.php`).
- `nexia-nextcloud-sdk-vfs`: recursive directory rename and move, recursive directory size and item count aggregation (`DirectorySizeSummary`), and typed WebDAV permissions parser (`NextcloudPermissions`).
- `nexia-nextcloud-sync`: 3-way reconciliation engine, persistent synchronization journal (`SyncJournal`), and thread-safe observable download queue (`SyncDownloadQueue`) with per-task progress metrics and snapshot streaming.
- `nexia-nextcloud-shares`: public link helpers (`create_public_link`, `get_public_link`, `get_or_create_public_link`, `delete_public_link`, and `Share::public_url`).

### Fixed

- The structured-logging assertion no longer flakes under parallel test
  execution. `tracing` caches each callsite's interest process-wide, and a
  thread without a subscriber resolves that cache to `Interest::never`, so the
  assertion now lives in its own integration-test binary
  (`tests/structured_logging.rs`) where no sibling test can poison the callsites
  it observes.

## 0.1.1

First release covering the whole workspace.

### Breaking

- The generic `sdk-*` crates moved under the Nextcloud namespace:
  `sdk-core` → `nextcloud-sdk-core`, `sdk-session` → `nexia-nextcloud-sdk-session`,
  `sdk-request` → `nexia-nextcloud-sdk-request`, `sdk-storage` → `nexia-nextcloud-sdk-storage`,
  `sdk-keyring` → `nexia-nextcloud-sdk-keyring`, `sdk-ratelimit` → `nexia-nextcloud-sdk-ratelimit`,
  `sdk-vfs` → `nexia-nextcloud-sdk-vfs`, `sdk-fuse` → `nexia-nextcloud-sdk-fuse`.
  Rust import paths change from `sdk_core::…` to `nextcloud_sdk_core::…`.
- The superseded `sdk-*` versions and `nexia-nextcloud-auth` 0.1.0 are yanked.

### Added

- Public `0.1.1` releases of every workspace crate: auth, transport, OCS, DAV,
  users, shares, notifications, activity, trashbin, versions, uploads, preview,
  deck, talk, client-config, kaccounts, goa, the session/request/storage/keyring/
  rate-limit/VFS/FUSE core crates, and the facade crate `nextcloud-rust-sdk`.
