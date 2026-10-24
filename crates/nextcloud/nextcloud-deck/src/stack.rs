//! Stack models for the Nextcloud Deck REST API.

use serde::{Deserialize, Serialize};

use crate::card::Card;

/// A stack (column) within a Nextcloud Deck board.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Stack {
    /// Unique identifier of the stack.
    pub id: u64,

    /// Title of the stack (e.g. `"To Do"`, `"Done"`).
    pub title: String,

    /// Identifier of the board this stack belongs to.
    #[serde(rename = "boardId", alias = "board_id")]
    pub board_id: u64,

    /// Display order of the stack within its board.
    pub order: i64,

    /// Cards currently contained within this stack.
    #[serde(default)]
    pub cards: Vec<Card>,

    /// Deletion timestamp (0 or Unix timestamp if deleted).
    #[serde(rename = "deletedAt", alias = "deleted_at", default)]
    pub deleted_at: Option<i64>,
}

impl Stack {
    /// Creates a new `Stack` with the provided parameters.
    pub fn new(
        id: u64,
        title: impl Into<String>,
        board_id: u64,
        order: i64,
        cards: Vec<Card>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            board_id,
            order,
            cards,
            deleted_at: None,
        }
    }
}
