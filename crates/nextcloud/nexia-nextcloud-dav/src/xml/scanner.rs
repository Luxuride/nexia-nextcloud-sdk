//! Locating elements in a document by qualified name.

use std::ops::Range;

/// Byte ranges describing one located element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    /// Range of the element's content, excluding the tags themselves.
    pub inner: Range<usize>,
    /// Index just past the element, where the next search continues.
    pub resume: usize,
}

/// Finds the next `<name>` element at or after `from`.
///
/// Names must match exactly, including the namespace prefix: `d:prop` never
/// matches `oc:prop` or `d:propstat`. Markup is consumed tag by tag, so a
/// `<` inside an attribute value or a comment is never mistaken for the
/// start of an element.
pub(crate) fn find(xml: &str, from: usize, name: &str) -> Option<Span> {
    let mut cursor = from.min(xml.len());
    while let Some(offset) = xml[cursor..].find('<') {
        let start = cursor + offset;
        if let Some(after) = skip_non_element(xml, start) {
            cursor = after;
            continue;
        }
        let end = tag_end(xml, start)?;
        if let Some(is_self_closing) = tag_is(xml, start, end, name) {
            let inner_start = end + 1;
            if is_self_closing {
                return Some(Span {
                    inner: inner_start..inner_start,
                    resume: inner_start,
                });
            }
            let close = format!("</{name}>");
            let offset = xml[inner_start..].find(&close)?;
            let inner_end = inner_start + offset;
            return Some(Span {
                inner: inner_start..inner_end,
                resume: inner_end + close.len(),
            });
        }
        cursor = end + 1;
    }
    None
}

/// Skips a comment, a CDATA section or a processing instruction, whose content
/// is not markup and must not be scanned for elements.
fn skip_non_element(xml: &str, start: usize) -> Option<usize> {
    let rest = &xml[start..];
    let (marker, terminator) = [
        ("<!--", "-->"),
        ("<![CDATA[", "]]>"),
        ("<?", "?>"),
        ("<!", ">"),
    ]
    .into_iter()
    .find(|(marker, _)| rest.starts_with(marker))?;
    let offset = rest[marker.len()..].find(terminator)?;
    Some(start + marker.len() + offset + terminator.len())
}

/// Index of the `>` closing the tag that starts at `start`.
///
/// Attribute values are honoured, so a `>` inside a quoted value does not end
/// the tag early.
fn tag_end(xml: &str, start: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (index, character) in xml[start + 1..].char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), closing) if open == closing => quote = None,
            _ => {}
        }
        if quote.is_none() && character == '>' {
            return Some(start + 1 + index);
        }
    }
    None
}

/// `Some(is_self_closing)` when the tag spanning `start..=end` opens `name`.
///
/// `None` means the tag is a different element or a closing tag, so the
/// search moves on past it.
fn tag_is(xml: &str, start: usize, end: usize, name: &str) -> Option<bool> {
    let body = &xml[start + 1..end];
    if body.starts_with('/') || body.starts_with('!') {
        return None;
    }
    let self_closing = body.ends_with('/');
    let name_end = body
        .find(|c: char| c == '/' || c.is_whitespace())
        .unwrap_or(body.len());
    (body[..name_end] == *name).then_some(self_closing)
}

#[cfg(test)]
mod tests {
    use super::find;

    #[test]
    fn finds_content_between_the_tags() {
        let xml = "<d:multistatus><d:response>x</d:response></d:multistatus>";
        let span = find(xml, 0, "d:response").expect("element is present");
        assert_eq!(&xml[span.inner.clone()], "x");
        assert_eq!(span.resume, xml.len() - "</d:multistatus>".len());
    }

    #[test]
    fn reports_self_closing_elements_as_empty() {
        let xml = "<d:prop><oc:fileid/></d:prop>";
        let span = find(xml, 0, "oc:fileid").expect("element is present");
        assert!(span.inner.is_empty());
        assert_eq!(span.resume, xml.find("</d:prop>").expect("closing tag"));
    }

    #[test]
    fn distinguishes_prefixed_names() {
        let xml = "<oc:propstat>a</oc:propstat>";
        assert!(find(xml, 0, "d:propstat").is_none());
    }

    #[test]
    fn does_not_match_a_longer_name() {
        let xml = "<d:propstat>x</d:propstat>";
        assert!(find(xml, 0, "d:prop").is_none());
    }

    #[test]
    fn ignores_markup_inside_attribute_values() {
        let xml = r#"<a title="<d:prop>"><d:prop>v</d:prop></a>"#;
        let span = find(xml, 0, "d:prop").expect("element is present");
        assert_eq!(&xml[span.inner], "v");
    }

    #[test]
    fn skips_the_prolog_and_comments() {
        let xml = "<?xml version=\"1.0\"?><!-- <d:prop>comment</d:prop> --><d:prop>v</d:prop>";
        let span = find(xml, 0, "d:prop").expect("element is present");
        assert_eq!(&xml[span.inner], "v");
    }

    #[test]
    fn skips_cdata_sections() {
        let xml = "<a><![CDATA[<d:prop>no</d:prop>]]></a><d:prop>v</d:prop>";
        let span = find(xml, 0, "d:prop").expect("element is present");
        assert_eq!(&xml[span.inner], "v");
    }
}
