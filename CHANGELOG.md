# Changelog

All notable changes to the Nextcloud Rust SDK workspace.

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
