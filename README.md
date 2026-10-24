# nextcloud-rust-sdk

Rust client for the Nextcloud APIs.

## Layout

The SDK is a Cargo workspace of small crates, one per concern, split into two
trees: `crates/core/` holds the Nextcloud-agnostic SDK layer (anything that
would serve a different API client just as well), and `crates/nextcloud/`
holds everything that knows Nextcloud's wire format.

### Generic SDK layer (`crates/core/`)

| Crate | Responsibility |
| --- | --- |
| `sdk-core` | HTTP methods and status codes, redacting headers, `ServerUrl`/`ApiPath`, the `Transport` and `AuthProvider` seams, the staged `Error` type |
| `sdk-session` | Session lifecycle state machine: login, grant, authenticated, re-authentication |
| `sdk-request` | Per-call lifecycle state machine, retry policy and failure classification |

### Nextcloud layer (`crates/nextcloud/`)

| Crate | Responsibility |
| --- | --- |
| `nextcloud-auth` | App password, bearer token and anonymous credentials |
| `nextcloud-transport` | `reqwest`-backed implementation of `sdk_core::Transport` |
| `nextcloud-ocs` | OCS envelope (`meta`/`data`) and a client generic over transport and auth |
| `nextcloud-dav` | WebDAV paths, PROPFIND bodies and multistatus decoding |

The root `nextcloud-rust-sdk` package is the facade: one `Client` wiring the
layers together. Every API area lives there, one module tree per Nextcloud
endpoint family, and builds on the crates above rather than re-implementing
transport, credentials or envelope handling.

## Development

Build, test, format and lint inside the devcontainer:

```sh
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

## License

Licensed under the GNU General Public License, version 3 or later
(`GPL-3.0-or-later`). See [`LICENSE`](LICENSE).
