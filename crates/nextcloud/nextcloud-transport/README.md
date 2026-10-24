# nextcloud-transport

> Production HTTP transport implementation backed by reqwest for the Nextcloud Rust SDK.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-transport = "0.1.1"
```

## Features

- `ReqwestTransport`: Implements `nextcloud_sdk_core::Transport` with connection pooling and TLS.
- Integrated token-bucket upload and download bandwidth rate limiting.
- Real-time download and upload progress reporting via incremental callbacks.
- Configurable request, connect, and overall timeouts.

## Example

```rust,no_run
use nextcloud_transport::{BandwidthLimit, TransportBuilder};
use nextcloud_sdk_core::{ApiPath, HttpMethod, Request, ServerUrl, Transport};

let server = ServerUrl::parse("https://cloud.example.org")?;
let transport = TransportBuilder::with_server(server)
    .download_limit(BandwidthLimit::from_mib(10))
    .build()?;

let req = Request::new(HttpMethod::Get, ApiPath::ocs("cloud/capabilities")?);
let resp = transport.execute(req).await?;
assert!(resp.status().is_success());
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
