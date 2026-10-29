# nextcloud-auth

> Authentication credentials and header providers for the Nextcloud Rust SDK.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-auth = "0.1.2"
```

## Features

- `BasicAuth`: HTTP Basic authentication for app passwords and user credentials.
- `BearerAuth`: OAuth2 bearer token authentication.
- `Anonymous`: Anonymous access injecting mandatory Nextcloud OCS headers.
- `SecretString`: Zeroizing wrapper redacting secrets from debug logs.

## Example

```rust,no_run
use nextcloud_auth::{BasicAuth, SecretString};
use nextcloud_sdk_core::AuthProvider;

let auth = BasicAuth::new("alice", SecretString::new("app-password"));
let mut headers = nextcloud_sdk_core::Headers::new();
auth.authenticate(&mut headers);
assert!(headers.get("Authorization").is_some());
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
