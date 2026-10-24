//! An element located in a document.

use super::escape::unescape_text;
use super::response::{elements, first_element, has_element};

/// An element located in a document: its qualified name and its content.
///
/// A handle, not a tree node: it borrows the document slice it was found in,
/// so chained lookups cost no allocation and nothing is copied until
/// [`text`](Self::text) is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element<'a> {
    name: &'a str,
    inner: &'a str,
}

impl<'a> Element<'a> {
    /// Wraps a located element; callers obtain one through
    /// [`elements`](super::response::elements) or
    /// [`first_element`](super::response::first_element).
    pub(crate) const fn new(name: &'a str, inner: &'a str) -> Self {
        Self { name, inner }
    }

    /// Qualified name the element was found by, e.g. `d:propstat`.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Content between the tags, markup included.
    pub const fn inner(&self) -> &'a str {
        self.inner
    }

    /// Content as text: entities decoded and surrounding whitespace trimmed.
    pub fn text(&self) -> String {
        unescape_text(self.inner).trim().to_owned()
    }

    /// First child element with `name`.
    pub fn child(&self, name: &'a str) -> Option<Self> {
        first_element(self.inner, name)
    }

    /// Every child element with `name`, in document order.
    ///
    /// "Child" is scanning order, not strict nesting: a match inside another
    /// element of a different name is still returned.
    pub fn children(&self, name: &'a str) -> Vec<Self> {
        elements(self.inner, name)
    }

    /// Whether an element with `name` appears anywhere in the content.
    pub fn has(&self, name: &'a str) -> bool {
        has_element(self.inner, name)
    }
}

#[cfg(test)]
mod tests {
    use crate::xml::response::first_element;

    const BODY: &str = "<d:response><d:href>/a</d:href><d:propstat><d:prop>\
        <oc:fileid>7</oc:fileid></d:prop></d:propstat></d:response>";

    fn response() -> super::Element<'static> {
        first_element(BODY, "d:response").expect("response is present")
    }

    #[test]
    fn chained_lookups_walk_into_nested_elements() {
        let propstat = response().child("d:propstat").expect("propstat present");
        let fileid = propstat
            .child("d:prop")
            .and_then(|prop| prop.child("oc:fileid"))
            .map(|element| element.text());
        assert_eq!(fileid.as_deref(), Some("7"));
    }

    #[test]
    fn name_and_inner_expose_the_located_slice() {
        let element = response();
        assert_eq!(element.name(), "d:response");
        assert!(element.inner().starts_with("<d:href>"));
    }

    #[test]
    fn has_reports_absence_rather_than_guessing() {
        assert!(response().has("d:propstat"));
        assert!(!response().has("d:collection"));
    }
}
