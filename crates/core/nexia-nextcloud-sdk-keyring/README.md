# nexia-nextcloud-sdk-keyring

> Multiplatform secure credential storage abstraction with Flatpak portal fallback and native desktop keychain support.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-sdk-keyring = "0.2.1"
```

## Features

- Linux & Flatpak: Uses `oo7` to communicate with FreeDesktop Secret Service and `org.freedesktop.portal.Secret` inside Flatpak.
- macOS: Uses native Apple Keychain Services (`apple-native`).
- Windows: Uses native Windows Credential Manager (`windows-native`).
- `MemoryKeyring`: Fully thread-safe in-memory backend for deterministic unit tests.
- `SecretString`: Zeroizes memory on drop and redacts secrets from `Debug` and `Display`.

## Example

```rust,no_run
use nexia_nextcloud_sdk_keyring::{KeyringBackend, KeyringEntry, PlatformKeyring, SecretString};

let entry = KeyringEntry::new("org.example.NextcloudApp", "default_account");
entry.set_password(SecretString::new("super-secret-password")).await?;

let retrieved = entry.get_password().await?;
assert_eq!(retrieved.map(|s| s.expose().to_string()), Some("super-secret-password".to_string()));
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
