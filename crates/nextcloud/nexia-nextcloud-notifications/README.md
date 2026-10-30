# nexia-nextcloud-notifications

> Nextcloud Notifications v2 API client for receiving and dismissing user notifications.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-notifications = "0.2.1"
```

## Features

- `NotificationClient`: Queries and dismisses user notifications.
- `Notification`: Details application origin, subject, message, link, and interactive actions.
- `NotificationAction`: Interactive button or link actions defined by Nextcloud apps.

## Example

```rust,no_run
use nexia_nextcloud_notifications::NotificationClient;

// Lists notifications, retrieves single notifications, and executes notification actions
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
