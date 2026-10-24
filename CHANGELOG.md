# Changelog

All notable changes to the Nextcloud Rust SDK workspace.

## Unreleased

### Added

- Zero-cost structured logging with `tracing` across all workspace crates and the facade client (`trace!`, `debug!`, `info!`, `warn!`, `error!`), with zero credential leakage.

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
