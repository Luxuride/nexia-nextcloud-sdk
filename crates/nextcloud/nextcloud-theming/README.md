# nextcloud-theming

> Nextcloud theming, custom client branding metadata, and unauthenticated server status.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-theming = "0.1.2"
```

## Features

- `ThemingClient`: Fetches server theming metadata (`/ocs/v2.php/apps/theming/api/v1/theme`).
- `CustomBranding`: Configures client application branding and custom `User-Agent`.
- `EffectiveBranding`: Unifies server theme with custom client overrides.
- `ServerStatusInfo`: Queries unauthenticated server status (`/status.php`).

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
