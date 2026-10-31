//! Nextcloud Talk (Spreed) API client and domain models.
//!
//! Provides strongly typed representations and an asynchronous client for
//! interacting with Nextcloud Talk conversations (rooms) and chat messages via
//! Nextcloud's OCS endpoints:
//! - [`Room`] and [`RoomType`] — conversations, access levels, and metadata beneath
//!   `/ocs/v2.php/apps/spreed/api/v4/room`
//! - [`ChatMessage`] — chat messages and system events beneath
//!   `/ocs/v2.php/apps/spreed/api/v1/chat`
//! - [`TalkClient`] — client wrapping [`nexia_nextcloud_ocs::OcsClient`]
//!
//! Official documentation:
//! - [Nextcloud Talk API](https://nextcloud-talk.readthedocs.io/en/latest/)
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod message;
mod room;
mod room_type;
mod wire;

pub use client::{CHAT_API_PATH, ROOM_API_PATH, TalkClient};
pub use message::ChatMessage;
pub use room::Room;
pub use room_type::RoomType;
