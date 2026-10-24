//! A minimal XML writer: open, close, empty, text.

use super::escape::escape_text;

/// Builds XML by appending element markup to a string.
///
/// The writer treats names as opaque: the caller owns namespace prefixes and
/// their declarations, and closing an element is the caller's promise that it
/// was opened. That is all a fixed request body needs, and it keeps the
/// output byte-for-byte predictable in tests.
#[derive(Debug, Default)]
pub struct XmlWriter {
    output: String,
}

impl XmlWriter {
    /// An empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes the XML declaration.
    pub fn prolog(&mut self) {
        self.output
            .push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    }

    /// Opens `<name>`.
    pub fn open(&mut self, name: &str) {
        self.open_with(name, &[]);
    }

    /// Opens `<name>` with attributes in the order given.
    pub fn open_with(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.output.push('<');
        self.output.push_str(name);
        for (key, value) in attributes {
            self.output.push(' ');
            self.output.push_str(key);
            self.output.push_str("=\"");
            self.output.push_str(&escape_text(value));
            self.output.push('"');
        }
        self.output.push('>');
    }

    /// Closes `</name>`.
    pub fn close(&mut self, name: &str) {
        self.output.push_str("</");
        self.output.push_str(name);
        self.output.push('>');
    }

    /// Writes a self-closing `<name/>`.
    pub fn empty(&mut self, name: &str) {
        self.output.push('<');
        self.output.push_str(name);
        self.output.push_str("/>");
    }

    /// Writes escaped character data.
    pub fn text(&mut self, value: &str) {
        self.output.push_str(&escape_text(value));
    }

    /// The document built so far.
    pub fn as_str(&self) -> &str {
        &self.output
    }

    /// Consumes the writer, yielding the document.
    pub fn into_string(self) -> String {
        self.output
    }
}

#[cfg(test)]
mod tests {
    use super::XmlWriter;

    #[test]
    fn nests_elements() {
        let mut writer = XmlWriter::new();
        writer.open("d:prop");
        writer.empty("d:getetag");
        writer.close("d:prop");
        assert_eq!(writer.as_str(), "<d:prop><d:getetag/></d:prop>");
    }

    #[test]
    fn escapes_attribute_values_and_text() {
        let mut writer = XmlWriter::new();
        writer.open_with("e", &[("name", "a&b")]);
        writer.text("1 < 2");
        writer.close("e");
        assert_eq!(writer.as_str(), "<e name=\"a&amp;b\">1 &lt; 2</e>");
    }
}
