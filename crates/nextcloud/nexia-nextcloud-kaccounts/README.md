# nexia-nextcloud-kaccounts

> KDE Plasma KAccounts integration for Nextcloud system single-sign-on.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nexia-nextcloud-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-kaccounts = "0.2.1"
```

## Features

- Reads KDE Accounts-SSO SQLite database (`accounts.db`) in read-only mode.
- Extracts Nextcloud / ownCloud server URLs, usernames, and enabled services.
- Resolves stored credentials from the KDE Wallet via `nexia-nextcloud-sdk-keyring`.

## Example

```rust,no_run
use nexia_nextcloud_kaccounts::KAccountsDatabase;

// Discovers and queries KDE Plasma Online Accounts for Nextcloud accounts
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
