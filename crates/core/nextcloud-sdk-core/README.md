# nextcloud-sdk-core

> Core primitives, type-safe path abstractions, and seam traits for the Nextcloud Rust SDK.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-sdk-core = "0.1.1"
```

## Features

- `ServerUrl`: Validated, normalized Nextcloud base URL.
- `ApiPath`: Strictly validated endpoint path (OCS, DAV, or public).
- `Transport`: Core trait for asynchronous HTTP execution.
- `AuthProvider`: Core trait for injecting credentials into request headers.
- `Header` / `Headers`: Case-insensitive HTTP header map with automatic credential redaction.
- `Error`: Typed SDK error classifying network, HTTP status, decoding, and URL errors.

## Example

```rust,no_run
use nextcloud_sdk_core::{ApiPath, HttpMethod, Request, ServerUrl};

let server = ServerUrl::parse("https://cloud.example.org")?;
let path = ApiPath::dav("files/alice/Photos")?;
let request = Request::new(HttpMethod::Get, path);
assert_eq!(request.method(), HttpMethod::Get);
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
