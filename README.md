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
| `nextcloud-sdk-core` | HTTP methods and status codes, redacting headers, `ServerUrl`/`ApiPath`, the `Transport` and `AuthProvider` seams, the staged `Error` type |
| `nextcloud-sdk-session` | Session lifecycle state machine: login, grant, authenticated, re-authentication |
| `nextcloud-sdk-request` | Per-call lifecycle state machine, retry policy and failure classification |
| `nextcloud-sdk-storage` | Encrypted SQLCipher dual SQLite storage (`auth.db` & `cache.db`) and secure key store |
| `nextcloud-sdk-keyring` | Multi-platform secure keyring abstraction (Linux FreeDesktop + Flatpak portal via `oo7`, macOS Keychain, Windows Credential Manager, Memory store) |
| `nextcloud-sdk-vfs` | Transient virtual file system tree, node metadata, multi-resolution thumbnails, and hydration lifecycle |
| `nextcloud-sdk-fuse` | Transient VFS FUSE driver for desktop filesystem mounting, inode allocation, and on-demand hydration |

### Nextcloud layer (`crates/nextcloud/`)

| Crate | Responsibility |
| --- | --- |
| `nextcloud-auth` | App password, bearer token and anonymous credentials |
| `nextcloud-transport` | `reqwest`-backed implementation of `nextcloud_sdk_core::Transport` |
| `nextcloud-ocs` | OCS envelope (`meta`/`data`), mutations (POST, PUT, DELETE), and generic client |
| `nextcloud-dav` | WebDAV paths, file & collection CRUD operations (`get`, `put`, `mkcol`, `delete`, `move_`, `copy`), PROPFIND bodies and multistatus decoding |
| `nextcloud-users` | Server capabilities and user profile data |
| `nextcloud-shares` | OCS file sharing API, public links, share types, and permissions bitflags |
| `nextcloud-notifications` | Nextcloud Notifications v2 API and notification actions |
| `nextcloud-activity` | Nextcloud Activity v2 API streams and filter queries |
| `nextcloud-trashbin` | WebDAV trashbin client: listing, restoring, and permanently deleting items |
| `nextcloud-versions` | WebDAV file versioning client: revision history listing, downloading, and restoring revisions |
| `nextcloud-uploads` | WebDAV chunked upload streaming v2 client (`/remote.php/dav/uploads/{user}/...`) |
| `nextcloud-deck` | Nextcloud Deck Kanban boards, stacks, cards, and labels (`/apps/deck/api/...`) |
| `nextcloud-talk` | Nextcloud Talk (Spreed) conversations, chat messages, and rooms (`/apps/spreed/api/...`) |
| `nextcloud-preview` | Nextcloud thumbnail and preview generation client (`/core/preview.png`) |
| `nextcloud-goa` | Direct GNOME Online Accounts (`org.gnome.OnlineAccounts`) D-Bus client via `zbus` for desktop SSO |
| `nextcloud-kaccounts` | KDE Plasma Online Accounts (KAccounts / Accounts-SSO) reader for Nextcloud system accounts |
| `nextcloud-client-config` | Official Nextcloud desktop client profile discovery and config parser (`nextcloud.cfg`) across Linux, macOS, and Windows |

The root `nextcloud-rust-sdk` package is the facade: one `Client` wiring the
layers together, including a unified `DesktopSso` discovery coordinator that
aggregates accounts across GNOME, KDE, and desktop client profiles. Every API area lives there, one module tree per Nextcloud
endpoint family, and builds on the crates above rather than re-implementing
transport, credentials or envelope handling.

## Contributing

We welcome both human and LLM / AI-assisted contributions! Please review [`CONTRIBUTING.md`](CONTRIBUTING.md) for quality gates, code rules, and the required LLM PR submission template.

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
