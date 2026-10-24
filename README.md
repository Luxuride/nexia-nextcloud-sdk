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
| `sdk-storage` | Encrypted SQLCipher dual SQLite storage (`auth.db` & `cache.db`) and FreeDesktop Secret Service (`oo7`) key store |
| `sdk-vfs` | Transient virtual file system tree, node metadata, multi-resolution thumbnails, and hydration lifecycle |

### Nextcloud layer (`crates/nextcloud/`)

| Crate | Responsibility |
| --- | --- |
| `nextcloud-auth` | App password, bearer token and anonymous credentials |
| `nextcloud-transport` | `reqwest`-backed implementation of `sdk_core::Transport` |
| `nextcloud-ocs` | OCS envelope (`meta`/`data`), mutations (POST, PUT, DELETE), and generic client |
| `nextcloud-dav` | WebDAV paths, file & collection CRUD operations (`get`, `put`, `mkcol`, `delete`, `move_`, `copy`), PROPFIND bodies and multistatus decoding |
| `nextcloud-users` | Server capabilities and user profile data |
| `nextcloud-shares` | OCS file sharing API, public links, share types, and permissions bitflags |
| `nextcloud-notifications` | Nextcloud Notifications v2 API and notification actions |
| `nextcloud-activity` | Nextcloud Activity v2 API streams and filter queries |
| `nextcloud-trashbin` | WebDAV trashbin client: listing, restoring, and permanently deleting items |
| `nextcloud-versions` | WebDAV file versioning client: revision history listing, downloading, and restoring revisions |
| `nextcloud-uploads` | WebDAV chunked upload streaming v2 client (`/remote.php/dav/uploads/{user}/...`) |
| `nextcloud-preview` | Nextcloud thumbnail and preview generation client (`/core/preview.png`) |
| `nextcloud-goa` | Direct GNOME Online Accounts (`org.gnome.OnlineAccounts`) D-Bus client via `zbus` for desktop SSO |

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
