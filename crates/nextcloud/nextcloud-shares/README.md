# nextcloud-shares

> OCS file sharing and public links management client for Nextcloud.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-shares = "0.1.2"
```

## Features

- `ShareClient`: Lists, retrieves, creates, updates, and deletes shares.
- `CreateShare`: Builder for public link shares, user shares, and group shares.
- `SharePermissions`: Bitflags configuring read, update, create, delete, and share rights.

## Example

```rust,no_run
use nextcloud_shares::{CreateShare, ShareClient, SharePermissions};

// Creates public link shares, user shares, and group shares with permissions and expiration
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
