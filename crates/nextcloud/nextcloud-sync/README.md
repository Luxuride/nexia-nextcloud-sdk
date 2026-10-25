# nextcloud-sync

> Bidirectional synchronization engine, reconciliation logic, persistent sync journal, and observable download queue for Nextcloud.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-sync = "0.1.2"
```

## Features

- `reconcile`: Deterministic 3-way synchronization reconciliation engine (`SyncAction`).
- `SyncJournal`: Persistent synchronization journal tracking remote ETags, local mtimes, and sizes.
- `scan_local_directory`: Recursive local filesystem scanner.
- `SyncDownloadQueue`: Thread-safe, observable download queue with priority scheduling, per-task progress metrics (`DownloadProgress`), aggregate snapshots (`DownloadQueueSnapshot`), and event streaming (`DownloadQueueEvent`).

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
