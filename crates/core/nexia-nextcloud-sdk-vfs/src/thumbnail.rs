//! Multi-resolution thumbnail caching for virtual nodes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Cache of rendered thumbnails keyed by resolution `(width, height)`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailCache {
    entries: BTreeMap<(u32, u32), Vec<u8>>,
}

impl ThumbnailCache {
    /// Creates an empty thumbnail cache.
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Stores a thumbnail for the specified dimensions.
    pub fn insert(&mut self, width: u32, height: u32, bytes: Vec<u8>) {
        self.entries.insert((width, height), bytes);
    }

    /// Retrieves thumbnail bytes for the specified dimensions, if cached.
    pub fn get(&self, width: u32, height: u32) -> Option<&[u8]> {
        self.entries.get(&(width, height)).map(Vec::as_slice)
    }

    /// Returns `true` if a thumbnail is cached for the specified dimensions.
    pub fn contains(&self, width: u32, height: u32) -> bool {
        self.entries.contains_key(&(width, height))
    }

    /// Removes and returns the thumbnail for the given dimensions, if present.
    pub fn remove(&mut self, width: u32, height: u32) -> Option<Vec<u8>> {
        self.entries.remove(&(width, height))
    }

    /// Clears all cached thumbnails.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns `true` if no thumbnails are cached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of cached resolutions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns an iterator over all cached thumbnail dimensions and byte slices.
    pub fn iter(&self) -> impl Iterator<Item = ((u32, u32), &[u8])> {
        self.entries
            .iter()
            .map(|(&(w, h), bytes)| ((w, h), bytes.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::ThumbnailCache;

    #[test]
    fn insert_and_retrieve_thumbnails() {
        let mut cache = ThumbnailCache::new();
        assert!(cache.is_empty());

        cache.insert(64, 64, vec![1, 2, 3]);
        cache.insert(256, 256, vec![4, 5, 6]);

        assert_eq!(cache.len(), 2);
        assert!(cache.contains(64, 64));
        assert_eq!(cache.get(64, 64), Some([1, 2, 3].as_slice()));
        assert_eq!(cache.get(256, 256), Some([4, 5, 6].as_slice()));
        assert_eq!(cache.get(128, 128), None);

        assert_eq!(cache.remove(64, 64), Some(vec![1, 2, 3]));
        assert!(!cache.contains(64, 64));
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }
}
