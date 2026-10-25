# nextcloud-ocs

> Nextcloud OCS v1/v2 API envelope parser and query engine.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-ocs = "0.1.2"
```

## Features

- `OcsEnvelope<T>`: Strongly typed deserializer for Nextcloud's `{ ocs: { meta: ..., data: ... } }` payload.
- `OcsClient`: Provides typed `get()`, `post()`, `put()`, and `delete()` operations with mandatory headers.
- `Query`: Query string builder for pagination, filtering, and sorting parameters.

## Example

```rust,no_run
use nextcloud_ocs::{OcsClient, OcsEnvelope};
use nextcloud_sdk_core::ApiPath;

// Queries Nextcloud OCS endpoints and decodes standard JSON responses
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
