//! Nextcloud Trashbin WebDAV API client.
//!
//! Provides [`TrashClient`] for interacting with a user's trashbin via WebDAV,
//! and [`TrashItem`] representing deleted files and folders.
//!
//! Endpoints wrapped:
//! - `PROPFIND /remote.php/dav/trashbin/{user}/trash/` to list deleted items
//! - `MOVE /remote.php/dav/trashbin/{user}/trash/{id}` to restore an item
//! - `DELETE /remote.php/dav/trashbin/{user}/trash/{id}` to permanently delete an item
//! - `DELETE /remote.php/dav/trashbin/{user}/trash/` to empty the trashbin
//!
//! Official documentation:
//! <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/WebDAV/trashbin.html>

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
mod item;
mod path;
mod propfind;

#[cfg(test)]
mod tests;

pub use client::TrashClient;
pub use item::TrashItem;
