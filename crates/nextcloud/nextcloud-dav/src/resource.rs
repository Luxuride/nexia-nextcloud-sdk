//! One `<d:response>` turned into a typed resource.

use crate::propfind::DavProperty;
use crate::xml::response::{has_element, text_of};
use crate::xml::{Element, MissingElement};

/// One entry of a multistatus answer.
///
/// Optional fields stay `None` both when the server omitted the property and
/// when it sent a value this crate could not read; the two are not
/// distinguished, so a caller must treat `None` as "unknown", never as
/// "false" or "zero".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DavResource {
    /// Path of the resource as the server reported it, usually
    /// percent-encoded and including `/remote.php/dav`.
    pub href: String,
    /// Whether the resource is a collection (`<d:collection/>`).
    pub is_collection: bool,
    /// `d:getetag`, verbatim including any quoting the server sent.
    pub etag: Option<String>,
    /// `d:getcontentlength`, in bytes.
    pub content_length: Option<u64>,
    /// `d:getcontenttype`.
    pub content_type: Option<String>,
    /// `oc:fileid`.
    pub file_id: Option<String>,
    /// `nc:has-preview`.
    pub has_preview: Option<bool>,
}

impl DavResource {
    /// Builds a resource from one `<d:response>` element.
    ///
    /// Only `d:href` is required: without it the entry does not identify
    /// anything. Property values are read from every `<d:propstat>` whose
    /// status is `200` (or that carries no status at all), because RFC 4918
    /// lets a server split requested properties across several propstat
    /// blocks by outcome.
    pub(crate) fn from_response(response: &Element<'_>) -> Result<Self, MissingElement> {
        let href = response
            .child("d:href")
            .map(|element| element.text())
            .filter(|href| !href.is_empty())
            .ok_or_else(|| MissingElement::new("d:href"))?;

        let props: Vec<Element<'_>> = response
            .children("d:propstat")
            .into_iter()
            .filter(is_successful_propstat)
            .filter_map(|propstat| propstat.child("d:prop"))
            .collect();

        let find = |property: DavProperty| {
            props
                .iter()
                .find_map(|prop| text_of(prop.inner(), property.qualified_name()))
        };

        Ok(Self {
            href,
            is_collection: props
                .iter()
                .any(|prop| has_element(prop.inner(), "d:collection")),
            etag: find(DavProperty::Etag),
            content_length: find(DavProperty::ContentLength)
                .or_else(|| find(DavProperty::Size))
                .and_then(|value| value.trim().parse().ok()),
            content_type: find(DavProperty::ContentType),
            file_id: find(DavProperty::FileId),
            has_preview: find(DavProperty::HasPreview).map(|value| is_truthy(value.as_str())),
        })
    }
}

/// A propstat without a status is treated as successful, which is what the
/// specification's default (200) means; anything not `200` carries a
/// `<d:responsedescription>` instead of usable values.
fn is_successful_propstat(propstat: &Element<'_>) -> bool {
    match propstat.child("d:status") {
        None => true,
        Some(status) => status.text().contains(" 200 "),
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

impl std::fmt::Display for DavResource {
    /// Renders the href and whether the entry is a collection, which is all
    /// that is worth putting in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.is_collection {
            "collection"
        } else {
            "file"
        };
        write!(f, "{} ({kind})", self.href)
    }
}

#[cfg(test)]
mod tests {
    use super::DavResource;
    use crate::xml::response::elements;

    const FILE_RESPONSE: &str = r#"<d:response>
        <d:href>/remote.php/dav/files/alice/Q1%20%26%20Q2.txt</d:href>
        <d:propstat>
            <d:prop>
                <d:getetag>"abc123"</d:getetag>
                <d:getcontentlength>42</d:getcontentlength>
                <d:getcontenttype>text/plain</d:getcontenttype>
                <d:resourcetype/>
                <oc:fileid>17</oc:fileid>
                <nc:has-preview>false</nc:has-preview>
            </d:prop>
            <d:status>HTTP/1.1 200 OK</d:status>
        </d:propstat>
    </d:response>"#;

    const COLLECTION_RESPONSE: &str = r#"<d:response>
        <d:href>/remote.php/dav/files/alice/Photos</d:href>
        <d:propstat>
            <d:prop>
                <d:resourcetype><d:collection/></d:resourcetype>
                <oc:fileid>9</oc:fileid>
                <nc:has-preview>true</nc:has-preview>
            </d:prop>
            <d:status>HTTP/1.1 200 OK</d:status>
        </d:propstat>
    </d:response>"#;

    fn single(xml: &str) -> DavResource {
        let response = elements(xml, "d:response").remove(0);
        DavResource::from_response(&response).expect("resource decodes")
    }

    #[test]
    fn file_response_keeps_every_property() {
        let resource = single(FILE_RESPONSE);
        assert_eq!(
            resource.href,
            "/remote.php/dav/files/alice/Q1%20%26%20Q2.txt"
        );
        assert!(!resource.is_collection);
        assert_eq!(resource.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(resource.content_length, Some(42));
        assert_eq!(resource.content_type.as_deref(), Some("text/plain"));
        assert_eq!(resource.file_id.as_deref(), Some("17"));
        assert_eq!(resource.has_preview, Some(false));
    }

    #[test]
    fn collection_response_is_marked_as_such() {
        let resource = single(COLLECTION_RESPONSE);
        assert!(resource.is_collection);
        assert_eq!(resource.has_preview, Some(true));
        assert_eq!(resource.content_length, None);
    }

    #[test]
    fn unreadable_values_are_omitted_not_guessed() {
        let xml = r#"<d:response><d:href>/a</d:href><d:propstat><d:prop>
            <d:getcontentlength>not-a-number</d:getcontentlength>
            </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"#;
        assert_eq!(single(xml).content_length, None);
    }

    #[test]
    fn failed_propstat_contributes_nothing() {
        let xml = r#"<d:response><d:href>/a</d:href>
            <d:propstat><d:prop><d:getetag>"no"</d:getetag></d:prop>
                <d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>
        </d:response>"#;
        assert_eq!(single(xml).etag, None);
    }

    #[test]
    fn response_without_href_is_rejected() {
        let response = elements("<d:response><d:propstat/></d:response>", "d:response").remove(0);
        let error = DavResource::from_response(&response).expect_err("href is mandatory");
        assert_eq!(error.name(), "d:href");
    }

    #[test]
    fn display_mentions_kind_and_href() {
        assert_eq!(
            single(COLLECTION_RESPONSE).to_string(),
            "/remote.php/dav/files/alice/Photos (collection)"
        );
    }
}
