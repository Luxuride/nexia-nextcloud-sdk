# nexia-nextcloud-dav

> WebDAV file and directory CRUD operations and PROPFIND XML engine for Nextcloud.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nexia-nextcloud-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-dav = "0.2.1"
```

## Features

- `DavClient`: File CRUD operations (`get`, `put`, `mkcol`, `delete`, `copy`, `move_`).
- `PropfindBody`: Flexible PROPFIND request builder (`all()`, `names()`, explicit properties).
- `DavResource`: Parsed WebDAV resource metadata (size, content type, ETag, last modified).
- Streaming XML scanner and response parser without heavy external XML dependencies.

## Example

```rust,no_run
use nexia_nextcloud_dav::{DavClient, DavPath, Depth, PropfindBody};

// Lists resources, downloads files, uploads files, and moves/copies files over WebDAV
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
