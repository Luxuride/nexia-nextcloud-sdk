# nextcloud-sdk-storage

> Encrypted SQLite persistence and keychain-backed key management using SQLCipher for the Nextcloud Rust SDK.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-sdk-storage = "0.1.2"
```

## Features

- SQLCipher 256-bit AES encryption at rest.
- Dual-database architecture isolating sensitive credentials (`auth.db`) from volatile metadata (`cache.db`).
- `KeychainKeyStore`: Secure storage and retrieval of database encryption keys via platform keyrings.
- Automatic schema creation, verification, and migrations.

## Example

```rust,no_run
use nextcloud_sdk_storage::{EncryptedDatabase, StoragePaths, StoreKey};

let paths = StoragePaths::default_xdg()?;
let key = StoreKey::generate()?;
let auth_db = paths.open_auth_db(&key)?;
let cache_db = paths.open_cache_db(&key)?;
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
