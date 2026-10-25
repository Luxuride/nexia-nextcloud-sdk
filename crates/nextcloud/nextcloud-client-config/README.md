# nextcloud-client-config

> Desktop client configuration parser and zero-login account discovery for Nextcloud.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-client-config = "0.1.2"
```

## Features

- Discovers `nextcloud.cfg` paths on Linux (XDG + Flatpak), macOS, and Windows.
- Parses QSettings / INI format with indexed attributes (`0\url`, `0\user`).
- Resolves desktop client credentials from platform keyrings.

## Example

```rust,no_run
use nextcloud_client_config::discover_config_paths;

// Discovers and parses existing Nextcloud desktop client profiles across Linux, macOS, and Windows
let paths = discover_config_paths();
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
