# nextcloud-versions

> WebDAV file versioning history and restoration client for Nextcloud.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-versions = "0.1.2"
```

## Features

- `VersionsClient`: Interacts with `/remote.php/dav/versions/` endpoints.
- `FileVersion`: Historical file snapshot with timestamp, size, and version ID.
- Restores previous versions directly back into the user's active file tree.

## Example

```rust,no_run
use nextcloud_versions::VersionsClient;

// Lists historical file versions and restores files to earlier revisions
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
