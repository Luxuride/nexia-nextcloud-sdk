//! Card models for the Nextcloud Deck REST API.

use serde::{Deserialize, Deserializer, Serialize};

/// A card located within a Nextcloud Deck stack.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Card {
    /// Unique identifier of the card.
    pub id: u64,

    /// Title or summary of the card.
    pub title: String,

    /// Markdown-formatted card description.
    #[serde(default, deserialize_with = "deserialize_null_as_empty_string")]
    pub description: String,

    /// Identifier of the stack containing this card.
    #[serde(rename = "stackId", alias = "stack_id")]
    pub stack_id: u64,

    /// Identifier of the board containing this card, if populated.
    #[serde(rename = "boardId", alias = "board_id", default)]
    pub board_id: Option<u64>,

    /// Display order of the card within its stack.
    pub order: i64,

    /// Whether this card has been archived.
    #[serde(default)]
    pub archived: bool,

    /// Due date and time in ISO 8601 format (e.g. `"2026-10-30T19:00:00Z"`), if set.
    #[serde(rename = "duedate", alias = "due_date", alias = "dueDate", default)]
    pub due_date: Option<String>,

    /// Username of the card creator or owner.
    #[serde(default)]
    pub owner: Option<String>,

    /// Creation timestamp (Unix timestamp).
    #[serde(rename = "createdAt", alias = "created_at", default)]
    pub created_at: Option<i64>,

    /// Last modification timestamp (Unix timestamp).
    #[serde(rename = "lastModified", alias = "last_modified", default)]
    pub last_modified: Option<i64>,

    /// Deletion timestamp (0 or Unix timestamp if deleted).
    #[serde(rename = "deletedAt", alias = "deleted_at", default)]
    pub deleted_at: Option<i64>,
}

impl Card {
    /// Creates a new `Card` with the provided parameters.
    pub fn new(
        id: u64,
        title: impl Into<String>,
        description: impl Into<String>,
        stack_id: u64,
        order: i64,
        archived: bool,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            stack_id,
            board_id: None,
            order,
            archived,
            due_date: None,
            owner: None,
            created_at: None,
            last_modified: None,
            deleted_at: None,
        }
    }
}

fn deserialize_null_as_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}
