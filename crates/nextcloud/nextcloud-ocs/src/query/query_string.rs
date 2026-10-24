use super::OcsFormat;
use super::encoding::encode_component;

/// Ordered query parameters for an OCS call.
///
/// OCS reads parameters by name, so order does not matter to the server, but
/// insertion order is preserved here to keep a built query stable across runs
/// and comparable in tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    params: Vec<(String, String)>,
}

impl Query {
    /// Empty query.
    pub const fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Adds a parameter, replacing an existing one with the same name.
    ///
    /// Keys and values are percent-encoded when the query is built, so callers
    /// pass raw text: a value containing `&` or `=` stays one value.
    #[must_use]
    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        match self.params.iter_mut().find(|(name, _)| name == &key) {
            Some(entry) => entry.1 = value,
            None => self.params.push((key, value)),
        }
        self
    }

    /// Selects the response format.
    ///
    /// Equivalent to `.param("format", format)`, kept as its own method so the
    /// requirement for `format=json` is impossible to miss at a call site.
    #[must_use]
    pub fn format(self, format: OcsFormat) -> Self {
        self.param(OcsFormat::PARAM, format.as_str())
    }

    /// Number of parameters held.
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Whether no parameter is held.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Iterates over the parameters in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Renders the parameters as a query string, empty when there are none.
    ///
    /// The result starts with `?` so callers can concatenate it onto a path
    /// without branching.
    pub fn build(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.params {
            out.push(if out.is_empty() { '?' } else { '&' });
            out.push_str(&encode_component(key));
            out.push('=');
            out.push_str(&encode_component(value));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{OcsFormat, Query};

    #[test]
    fn empty_query_renders_nothing() {
        let query = Query::new();
        assert!(query.is_empty());
        assert_eq!(query.build(), "");
    }

    #[test]
    fn params_are_joined_with_ampersands() {
        let query = Query::new()
            .param("limit", "5")
            .param("offset", "10")
            .format(OcsFormat::Json);
        assert_eq!(query.build(), "?limit=5&offset=10&format=json");
        assert_eq!(query.len(), 3);
    }

    #[test]
    fn xml_format_is_selectable() {
        assert_eq!(Query::new().format(OcsFormat::Xml).build(), "?format=xml");
    }

    #[test]
    fn repeated_param_replaces_the_earlier_value() {
        let query = Query::new().param("limit", "5").param("limit", "50");
        assert_eq!(query.len(), 1);
        assert_eq!(query.build(), "?limit=50");
    }

    #[test]
    fn values_are_percent_encoded() {
        let query = Query::new().param("search", "a&b=c d");
        assert_eq!(query.build(), "?search=a%26b%3Dc%20d");
    }

    #[test]
    fn iterators_yield_raw_values() {
        let query = Query::new().param("user", "a b");
        let pairs: Vec<_> = query.iter().collect();
        assert_eq!(pairs, vec![("user", "a b")]);
    }
}
