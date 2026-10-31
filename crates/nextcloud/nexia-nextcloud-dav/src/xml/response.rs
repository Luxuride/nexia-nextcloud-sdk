//! Reading values out of a multistatus body.
//!
//! # What this reader is
//!
//! A targeted scanner: it finds an element by its exact qualified name and
//! yields the slice between its tags. Namespace prefixes are compared
//! literally and never resolved, which is sound for Nextcloud's WebDAV
//! answers because the server uses the same `d`/`oc`/`nc` prefixes this crate
//! writes.
//!
//! # What it deliberately is not
//!
//! - It does not expand namespace declarations, so a server answering with a
//!   different prefix (`<D:href>`) is not understood. The caller sees a
//!   missing element, never a wrong value.
//! - It does not validate well-formedness. It does skip comments, CDATA
//!   sections and processing instructions, and it respects quoted attribute
//!   values, so a `<` inside those cannot start a false match.
//! - It decodes only the five XML entities plus numeric character references
//!   (see [`escape`](super::escape)).
//!
//! Every lookup returns `Option`, so an unreadable property is omitted rather
//! than guessed at.

use super::scanner;
use super::{Element, MissingElement};

/// First element with `name`.
pub fn first_element<'a>(xml: &'a str, name: &'a str) -> Option<Element<'a>> {
    let span = scanner::find(xml, 0, name)?;
    Some(Element::new(name, &xml[span.inner]))
}

/// Every element with `name`, in document order.
///
/// A match nested inside another element of a different name is still
/// returned; see the module docs for why strict nesting is not attempted.
pub fn elements<'a>(xml: &'a str, name: &'a str) -> Vec<Element<'a>> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(span) = scanner::find(xml, cursor, name) {
        found.push(Element::new(name, &xml[span.inner]));
        cursor = span.resume;
    }
    found
}

/// Whether any element with `name` is present.
pub fn has_element(xml: &str, name: &str) -> bool {
    scanner::find(xml, 0, name).is_some()
}

/// Text of the first element with `name`, if it is present.
pub fn text_of(xml: &str, name: &str) -> Option<String> {
    first_element(xml, name).map(|element| element.text())
}

/// Requires `name` to be present, reporting it by name when absent.
pub fn require_element<'a>(xml: &'a str, name: &'a str) -> Result<Element<'a>, MissingElement> {
    first_element(xml, name).ok_or_else(|| MissingElement::new(name))
}

#[cfg(test)]
mod tests {
    use super::{elements, has_element, require_element, text_of};

    const BODY: &str = "<d:multistatus><d:response><d:href>/a&amp;b</d:href>\
        <d:propstat><d:prop><oc:fileid>7</oc:fileid></d:prop></d:propstat>\
        </d:response></d:multistatus>";

    #[test]
    fn reads_element_text_with_entities_decoded() {
        assert_eq!(text_of(BODY, "d:href").as_deref(), Some("/a&b"));
    }

    #[test]
    fn lists_every_matching_element_in_order() {
        let xml = "<a><b>1</b><b>2</b></a>";
        let texts: Vec<String> = elements(xml, "b").iter().map(|e| e.text()).collect();
        assert_eq!(texts, ["1", "2"]);
    }

    #[test]
    fn absent_elements_are_reported_not_invented() {
        assert!(!has_element(BODY, "d:collection"));
        assert!(text_of(BODY, "d:collection").is_none());
    }

    #[test]
    fn require_element_names_the_missing_tag() {
        assert!(require_element(BODY, "d:propstat").is_ok());
        let error = require_element(BODY, "d:collection").expect_err("collection is absent");
        assert_eq!(error.name(), "d:collection");
    }
}
