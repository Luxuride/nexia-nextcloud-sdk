# Changelog

All notable changes to the Nextcloud Rust SDK workspace.

## Unreleased

### Breaking

- `nextcloud-preview`: the FreeDesktop `Software` PNG chunk is no longer
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

## 0.1.2

### Added

- Zero-cost structured logging with `tracing` across all workspace crates and the facade client (`trace!`, `debug!`, `info!`, `warn!`, `error!`), with zero credential leakage.
- `nextcloud-theming`: theming metadata, custom client branding overrides, and unauthenticated server status endpoint (`/status.php`).
- `nextcloud-sdk-vfs`: recursive directory rename and move, recursive directory size and item count aggregation (`DirectorySizeSummary`), and typed WebDAV permissions parser (`NextcloudPermissions`).
- `nextcloud-sync`: 3-way reconciliation engine, persistent synchronization journal (`SyncJournal`), and thread-safe observable download queue (`SyncDownloadQueue`) with per-task progress metrics and snapshot streaming.
- `nextcloud-shares`: public link helpers (`create_public_link`, `get_public_link`, `get_or_create_public_link`, `delete_public_link`, and `Share::public_url`).

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
  `sdk-core` → `nextcloud-sdk-core`, `sdk-session` → `nextcloud-sdk-session`,
  `sdk-request` → `nextcloud-sdk-request`, `sdk-storage` → `nextcloud-sdk-storage`,
  `sdk-keyring` → `nextcloud-sdk-keyring`, `sdk-ratelimit` → `nextcloud-sdk-ratelimit`,
  `sdk-vfs` → `nextcloud-sdk-vfs`, `sdk-fuse` → `nextcloud-sdk-fuse`.
  Rust import paths change from `sdk_core::…` to `nextcloud_sdk_core::…`.
- The superseded `sdk-*` versions and `nextcloud-auth` 0.1.0 are yanked.

### Added

- Public `0.1.1` releases of every workspace crate: auth, transport, OCS, DAV,
  users, shares, notifications, activity, trashbin, versions, uploads, preview,
  deck, talk, client-config, kaccounts, goa, the session/request/storage/keyring/
  rate-limit/VFS/FUSE core crates, and the facade crate `nextcloud-rust-sdk`.
