# sdk-fuse

> Desktop FUSE filesystem driver for mounting transient Nextcloud virtual trees on Linux and macOS.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
sdk-fuse = "0.1.0"
```

## Features

- User-space filesystem driver exposing Nextcloud trees as local directories.
- Stable inode allocation and path lookup.
- Asynchronous on-demand file hydration on file read.

## Example

```rust,no_run
use sdk_fuse::{FuseDriver, InodeTable};
use sdk_vfs::VirtualTree;

let vfs = VirtualTree::new("Nextcloud");
let inodes = InodeTable::new();
// Mount with FUSE
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
