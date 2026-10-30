# nexia-nextcloud-sdk-session

> Pure state machine for Nextcloud session lifecycles and Login Flow v2.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nexia-nexia-nextcloud-sdk-session = "0.2.1"
```

## Features

- Pure transition table: no I/O, completely runtime-agnostic.
- Full Login Flow v2 support: initiation, polling, credential extraction, and reauth.
- Comprehensive state tracking: `Idle`, `AwaitingLogin`, `AwaitingGrant`, `Authenticated`, `ReauthRequired`.

## Example

```rust,no_run
use nexia_nextcloud_sdk_session::{Command, SessionEvent, SessionMachine, SessionState};

let mut session = SessionMachine::new();
assert_eq!(session.state(), &SessionState::Idle);

let cmd = session.handle(SessionEvent::StartRequested)?;
assert_eq!(cmd, Command::StartLogin);
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
