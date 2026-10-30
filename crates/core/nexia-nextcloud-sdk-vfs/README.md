# nexia-nextcloud-sdk-vfs

> Transient virtual filesystem tree with on-demand hydration and thumbnail caching.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-sdk-vfs = "0.2.1"
```

## Features

- Hierarchical virtual filesystem tree with path normalization.
- Hydration states: placeholder metadata vs. fully hydrated in-memory buffers.
- Integrated LRU thumbnail cache for fast preview rendering.

## Example

```rust,no_run
use nexia_nextcloud_sdk_vfs::{VirtualTree, NodeMetadata};

let mut vfs = VirtualTree::new("Nextcloud");
vfs.insert_file("/Documents/notes.txt", NodeMetadata::placeholder(1024, "text/plain"))?;
assert!(vfs.get("/Documents/notes.txt").is_some());
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
