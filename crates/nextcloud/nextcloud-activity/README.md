# nextcloud-activity

> Activity streams and audit feed client for Nextcloud.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-activity = "0.1.1"
```

## Features

- `ActivityClient`: Fetches activity feeds from Nextcloud.
- `ActivityFilter`: Filter by object type, object ID, stream direction, and timestamp.
- `Activity`: Detailed activity record with app name, subject, message, and target objects.

## Example

```rust,no_run
use nextcloud_activity::{ActivityClient, ActivityFilter};

// Queries audit logs and activity streams with pagination and filtering
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
