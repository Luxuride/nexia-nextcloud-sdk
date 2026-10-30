# nexia-nextcloud-goa

> GNOME Online Accounts integration for Nextcloud system single-sign-on.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-goa = "0.2.1"
```

## Features

- Communicates with `org.gnome.OnlineAccounts` via D-Bus.
- Extracts server URLs, usernames, and enabled calendar/storage services.
- Retrieves app passwords directly from the GNOME Keyring.

## Example

```rust,no_run
use nexia_nextcloud_goa::GnomeOnlineAccounts;

// Queries GNOME Online Accounts over D-Bus for Nextcloud system accounts
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
