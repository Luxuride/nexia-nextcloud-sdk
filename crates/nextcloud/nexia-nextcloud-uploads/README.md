# nexia-nextcloud-uploads

> Nextcloud WebDAV Chunked Uploads v2 streaming client for reliable large file uploads.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-uploads = "0.2.1"
```

## Features

- `UploadClient`: Manages chunked upload directories and chunk transfers.
- `UploadSession`: Coordinates chunk boundaries, byte offsets, and session state.
- Automatic retry support and resume capability across network interruptions.

## Example

```rust,no_run
use nexia_nextcloud_uploads::UploadClient;

// Slices large files into chunks, uploads them sequentially, and commits the final file
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
