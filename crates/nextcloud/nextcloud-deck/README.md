# nextcloud-deck

> Nextcloud Deck client for Kanban boards, stacks, and cards.

Part of the [`nextcloud-rust-sdk`](https://github.com/Luxuride/nextcloud-rust-sdk) workspace.

## Installation

```toml
[dependencies]
nextcloud-deck = "0.1.1"
```

## Features

- `DeckClient`: Full CRUD for boards, stacks, and cards.
- `Board`, `Stack`, `Card`: Strongly typed models for Deck Kanban workflows.

## Example

```rust,no_run
use nextcloud_deck::DeckClient;

// Manages Deck boards, stacks, cards, labels, and assignments
```

## License

GNU General Public License v3.0 or later ([`GPL-3.0-or-later`](https://www.gnu.org/licenses/gpl-3.0.html)).
