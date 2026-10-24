//! Board models for the Nextcloud Deck REST API.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

/// Permissions granted on a Nextcloud Deck board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct BoardPermissions {
    /// Whether the user has permission to read the board.
    #[serde(rename = "PERMISSION_READ", default)]
    pub read: bool,

    /// Whether the user has permission to edit cards and content.
    #[serde(rename = "PERMISSION_EDIT", default)]
    pub edit: bool,

    /// Whether the user has permission to write changes.
    #[serde(rename = "PERMISSION_WRITE", default)]
    pub write: bool,

    /// Whether the user has permission to manage board settings and stacks.
    #[serde(rename = "PERMISSION_MANAGE", default)]
    pub manage: bool,

    /// Whether the user has permission to share the board with other users.
    #[serde(rename = "PERMISSION_SHARE", default)]
    pub share: bool,

    /// Whether the user has permission to delete the board.
    #[serde(rename = "PERMISSION_DELETE", default)]
    pub delete: bool,
}

impl BoardPermissions {
    /// Creates a new `BoardPermissions` instance with the specified flags.
    pub const fn new(
        read: bool,
        edit: bool,
        write: bool,
        manage: bool,
        share: bool,
        delete: bool,
    ) -> Self {
        Self {
            read,
            edit,
            write,
            manage,
            share,
            delete,
        }
    }
}

/// The owner of a Nextcloud Deck board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BoardOwner {
    /// Primary key identifier of the owner.
    #[serde(rename = "primaryKey", skip_serializing_if = "Option::is_none")]
    pub primary_key: Option<String>,

    /// Unique Nextcloud user ID of the owner.
    pub uid: String,

    /// Display name of the owner, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}

impl BoardOwner {
    /// Creates a new `BoardOwner` with the given user ID and optional display name.
    pub fn new(uid: impl Into<String>, displayname: Option<String>) -> Self {
        let uid = uid.into();
        Self {
            primary_key: Some(uid.clone()),
            uid,
            displayname,
        }
    }
}

impl<'de> Deserialize<'de> for BoardOwner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BoardOwnerVisitor;

        impl<'de> serde::de::Visitor<'de> for BoardOwnerVisitor {
            type Value = BoardOwner;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string username or an owner object")
            }

            fn visit_str<E>(self, value: &str) -> Result<BoardOwner, E>
            where
                E: serde::de::Error,
            {
                Ok(BoardOwner {
                    primary_key: None,
                    uid: value.to_owned(),
                    displayname: None,
                })
            }

            fn visit_map<M>(self, map: M) -> Result<BoardOwner, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct Helper {
                    #[serde(rename = "primaryKey", default)]
                    primary_key: Option<String>,
                    #[serde(default)]
                    uid: Option<String>,
                    #[serde(default)]
                    displayname: Option<String>,
                }

                let helper =
                    Helper::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(BoardOwner {
                    primary_key: helper.primary_key,
                    uid: helper.uid.unwrap_or_default(),
                    displayname: helper.displayname,
                })
            }
        }

        deserializer.deserialize_any(BoardOwnerVisitor)
    }
}

/// A Nextcloud Deck board.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Board {
    /// Unique identifier of the board.
    pub id: u64,

    /// Title of the board.
    pub title: String,

    /// Color hex code of the board (e.g. `"0800fd"`).
    #[serde(default)]
    pub color: Option<String>,

    /// Whether this board is archived.
    #[serde(default)]
    pub archived: bool,

    /// Owner details of the board.
    #[serde(default)]
    pub owner: Option<BoardOwner>,

    /// User permissions on this board.
    #[serde(default)]
    pub permissions: Option<BoardPermissions>,

    /// Last modification timestamp (Unix timestamp).
    #[serde(rename = "lastModified", alias = "last_modified", default)]
    pub last_modified: Option<i64>,

    /// Deletion timestamp (0 or Unix timestamp if deleted).
    #[serde(rename = "deletedAt", alias = "deleted_at", default)]
    pub deleted_at: Option<i64>,
}

impl Board {
    /// Creates a new `Board` with the provided parameters.
    pub fn new(id: u64, title: impl Into<String>, color: Option<String>, archived: bool) -> Self {
        Self {
            id,
            title: title.into(),
            color,
            archived,
            owner: None,
            permissions: None,
            last_modified: None,
            deleted_at: None,
        }
    }
}
