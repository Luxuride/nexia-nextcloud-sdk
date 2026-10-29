# nextcloud-talk

> Nextcloud Talk (Spreed) client for chat conversations and rooms.

Part of the [`nexia-nextcloud-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-talk = "0.1.2"
```

## Features

- `TalkClient`: Interacts with Nextcloud Talk (Spreed) API endpoints.
- `Room`: Public, private, group, and direct conversation rooms.
- `ChatMessage`: Structured chat message with formatting, author, and timestamp.

## Example

```rust,no_run
use nextcloud_talk::TalkClient;

// Lists chat rooms, sends chat messages, and inspects conversation participants
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
