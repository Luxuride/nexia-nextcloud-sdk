# nexia-nextcloud-trashbin

> WebDAV trashbin management and file restoration client for Nextcloud.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nexia-nextcloud-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-trashbin = "0.2.1"
```

## Features

- `TrashClient`: Interacts with `/remote.php/dav/trashbin/` endpoints.
- `TrashItem`: Parsed deleted resource with original path, deletion timestamp, and size.
- Restores deleted files with a single method call.

## Example

```rust,no_run
use nexia_nextcloud_trashbin::TrashClient;

// Lists deleted files, restores items to original paths, and permanently purges trash
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
