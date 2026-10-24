# nextcloud-preview

> Nextcloud thumbnail and image preview generation client.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-preview = "0.1.0"
```

## Features

- `PreviewClient`: Requests image previews from `/index.php/core/preview`.
- `PreviewOptions`: Configures target dimensions (width/height), crop mode, and format.

## Example

```rust,no_run
use nextcloud_preview::{PreviewClient, PreviewOptions};

// Requests scaled image previews and thumbnails by file ID
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
