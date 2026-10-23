use super::Header;

/// Ordered header collection with case-insensitive name lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    entries: Vec<Header>,
}

impl Headers {
    /// Empty collection.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Inserts a header, replacing any existing header with the same name.
    pub fn insert(&mut self, header: Header) {
        match self.position(header.name()) {
            Some(index) => self.entries[index] = header,
            None => self.entries.push(header),
        }
    }

    /// Builder form of [`Headers::insert`].
    #[must_use]
    pub fn with(mut self, header: Header) -> Self {
        self.insert(header);
        self
    }

    /// Inserts a header from its name and value.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.insert(Header::new(name, value));
    }

    /// Value of the first header with `name`, matched case-insensitively.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|header| header.name().eq_ignore_ascii_case(name))
            .map(Header::value)
    }

    /// Removes every header with `name`.
    pub fn remove(&mut self, name: &str) {
        self.entries
            .retain(|header| !header.name().eq_ignore_ascii_case(name));
    }

    /// Iterates over the headers in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &Header> {
        self.entries.iter()
    }

    /// Number of headers held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no header is held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|header| header.name().eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::{Header, Headers};

    #[test]
    fn lookup_ignores_name_case() {
        let headers = Headers::new().with(Header::new("content-type", "application/json"));
        assert_eq!(headers.get("Content-Type"), Some("application/json"));
    }

    #[test]
    fn insert_replaces_instead_of_appending() {
        let mut headers = Headers::new();
        headers.set("Depth", "1");
        headers.set("depth", "0");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("Depth"), Some("0"));
    }

    #[test]
    fn remove_drops_all_casing_variants() {
        let mut headers = Headers::new();
        headers.set("Accept", "application/json");
        headers.remove("accept");
        assert!(headers.is_empty());
    }
}
