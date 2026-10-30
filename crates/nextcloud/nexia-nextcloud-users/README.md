# nexia-nextcloud-users

> Server capabilities and user provisioning client for Nextcloud.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nexia-nextcloud-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-users = "0.2.1"
```

## Features

- `Capabilities`: Discovers active Nextcloud apps, storage limits, and WebDAV endpoints.
- `User`: User profile with storage quotas, email, display name, and group memberships.
- `Quota`: Storage allocation metrics handling unlimited and bounded quotas.

## Example

```rust,no_run
use nexia_nextcloud_users::UserClient;

// Queries server capabilities (WebDAV roots, chunking limits, theming) and user quotas
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
