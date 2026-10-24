//! `207 Multi-Status` body -> `Vec<DavResource>`.

use sdk_core::DecodeError;

use crate::resource::DavResource;
use crate::xml::MissingElement;
use crate::xml::response;

/// Type name reported by every [`DecodeError`] this module produces.
pub(crate) const TARGET: &str = "DavResource";

/// Decodes the `<d:response>` entries of a multistatus body.
///
/// A body with no `d:multistatus` element is an error: an empty list would be
/// indistinguishable from "the address holds nothing", which is a very
/// different answer. A single unreadable entry fails the whole decode rather
/// than being skipped, because a caller listing a folder must not silently
/// lose a child.
pub(crate) fn decode(body: &str) -> Result<Vec<DavResource>, DecodeError> {
    let multistatus = response::first_element(body, "d:multistatus")
        .ok_or_else(|| DecodeError::new(TARGET, MissingElement::new("d:multistatus")))?;
    multistatus
        .children("d:response")
        .iter()
        .map(|element| DavResource::from_response(element).map_err(|e| DecodeError::new(TARGET, e)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::decode;
    use crate::fixtures::TWO_ENTRIES;
    use crate::resource::DavResource;

    #[test]
    fn two_entry_body_decodes_into_a_collection_and_a_file() {
        let resources = decode(TWO_ENTRIES).expect("body decodes");
        assert_eq!(resources.len(), 2);

        let folder = &resources[0];
        assert!(folder.is_collection);
        assert_eq!(folder.href, "/remote.php/dav/files/alice/Photos");
        assert_eq!(folder.file_id.as_deref(), Some("9"));
        assert_eq!(folder.content_length, None);
        assert_eq!(folder.has_preview, Some(false));

        let file = &resources[1];
        assert!(!file.is_collection);
        assert_eq!(file.content_length, Some(128));
        assert_eq!(
            file.content_type.as_deref(),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(file.etag.as_deref(), Some("\"a1b2c3\""));
        assert_eq!(file.has_preview, Some(true));
    }

    #[test]
    fn empty_multistatus_decodes_to_no_resources() {
        let body = "<d:multistatus xmlns:d=\"DAV:\"></d:multistatus>";
        assert_eq!(decode(body).expect("body decodes").len(), 0);
    }

    #[test]
    fn missing_multistatus_is_a_decode_error_naming_the_target() {
        let error = decode("<html>Bad Gateway</html>").expect_err("not a multistatus");
        assert_eq!(error.target(), super::TARGET);
        assert!(error.to_string().contains("d:multistatus"));
    }

    #[test]
    fn entry_without_href_fails_the_decode() {
        let body = r#"<d:multistatus xmlns:d="DAV:"><d:response>
            <d:propstat><d:prop/></d:propstat></d:response></d:multistatus>"#;
        let error = decode(body).expect_err("href is mandatory");
        assert_eq!(error.target(), super::TARGET);
        assert!(error.to_string().contains("d:href"));
    }

    #[test]
    fn collection_flag_does_not_leak_between_entries() {
        let resources: Vec<DavResource> = decode(TWO_ENTRIES).expect("body decodes");
        assert_eq!(
            resources.iter().filter(|r| r.is_collection).count(),
            1,
            "only the folder is a collection"
        );
    }
}
