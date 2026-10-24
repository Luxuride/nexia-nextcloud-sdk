//! Hydration state tracking for virtual file contents.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Payload data backing a hydrated virtual file node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HydratedContent {
    /// In-memory byte buffer containing file content.
    Bytes(Vec<u8>),
    /// Reference to a locally cached file on disk.
    Path(PathBuf),
}

impl HydratedContent {
    /// Returns the content as a byte slice if loaded in memory.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes.as_slice()),
            Self::Path(_) => None,
        }
    }

    /// Returns the local cache path if backed by disk.
    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Self::Bytes(_) => None,
            Self::Path(path) => Some(path.as_path()),
        }
    }
}

impl From<Vec<u8>> for HydratedContent {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes)
    }
}

impl From<PathBuf> for HydratedContent {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

/// Hydration state representing whether file content is available or a transient placeholder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HydrationState {
    /// Metadata-only transient placeholder. No file content is loaded in memory.
    #[default]
    Placeholder,
    /// Content loaded with byte payload or cached file path reference.
    Hydrated(HydratedContent),
}

impl HydrationState {
    /// Returns `true` if this node is a metadata-only placeholder.
    pub const fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder)
    }

    /// Returns `true` if file content is hydrated (in-memory bytes or disk cache).
    pub const fn is_hydrated(&self) -> bool {
        matches!(self, Self::Hydrated(_))
    }

    /// Returns in-memory bytes if hydrated in memory.
    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Hydrated(content) => content.as_bytes(),
            Self::Placeholder => None,
        }
    }

    /// Returns cached disk path if hydrated via disk cache.
    pub fn cached_path(&self) -> Option<&Path> {
        match self {
            Self::Hydrated(content) => content.as_path(),
            Self::Placeholder => None,
        }
    }
}

impl From<Vec<u8>> for HydrationState {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Hydrated(HydratedContent::Bytes(bytes))
    }
}

impl From<PathBuf> for HydrationState {
    fn from(path: PathBuf) -> Self {
        Self::Hydrated(HydratedContent::Path(path))
    }
}

impl From<HydratedContent> for HydrationState {
    fn from(content: HydratedContent) -> Self {
        Self::Hydrated(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_behavior() {
        let state = HydrationState::Placeholder;
        assert!(state.is_placeholder());
        assert!(!state.is_hydrated());
        assert_eq!(state.bytes(), None);
        assert_eq!(state.cached_path(), None);
    }

    #[test]
    fn hydrated_bytes_behavior() {
        let state = HydrationState::from(b"hello world".to_vec());
        assert!(!state.is_placeholder());
        assert!(state.is_hydrated());
        assert_eq!(state.bytes(), Some(b"hello world".as_slice()));
        assert_eq!(state.cached_path(), None);
    }

    #[test]
    fn hydrated_path_behavior() {
        let path = PathBuf::from("/tmp/cache/notes.md");
        let state = HydrationState::from(path.clone());
        assert!(!state.is_placeholder());
        assert!(state.is_hydrated());
        assert_eq!(state.bytes(), None);
        assert_eq!(state.cached_path(), Some(path.as_path()));
    }
}
