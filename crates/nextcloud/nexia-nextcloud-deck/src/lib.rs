//! Nextcloud Deck REST API client and domain models.
//!
//! Provides types and methods for managing Deck boards, stacks, and cards
//! via the Nextcloud Deck REST API (`/index.php/apps/deck/api/v1.0/...`).

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod board;
mod card;
mod client;
mod stack;

pub use board::{Board, BoardOwner, BoardPermissions};
pub use card::Card;
pub use client::{DECK_API_ROOT, DeckClient};
pub use stack::Stack;
