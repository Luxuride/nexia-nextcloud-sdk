# nextcloud-kaccounts

> KDE Plasma KAccounts integration for Nextcloud system single-sign-on.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-kaccounts = "0.1.0"
```

## Features

- Reads KDE Accounts-SSO SQLite database (`accounts.db`) in read-only mode.
- Extracts Nextcloud / ownCloud server URLs, usernames, and enabled services.
- Resolves stored credentials from the KDE Wallet via `nextcloud-sdk-keyring`.

## Example

```rust,no_run
use nextcloud_kaccounts::KAccountsDatabase;

// Discovers and queries KDE Plasma Online Accounts for Nextcloud accounts
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
