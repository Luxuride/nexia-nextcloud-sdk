//! Decoding WebDAV multistatus XML into [`FileVersion`] items.

use nexia_nextcloud_core::DecodeError;
use nexia_nextcloud_dav::xml::{self, MissingElement};

use crate::version::FileVersion;

/// Target name used for decode errors.
const TARGET: &str = "FileVersion";

/// Decodes a WebDAV `207 Multi-Status` response body into a list of file versions.
///
/// Any entry representing the parent collection (either containing a collection
/// marker `<d:collection/>` or ending in the `file_id` path segment) is omitted
/// so that only individual revisions are returned.
pub(crate) fn decode_multistatus(
    body: &str,
    file_id: u64,
) -> Result<Vec<FileVersion>, DecodeError> {
    let multistatus = xml::first_element(body, "d:multistatus")
        .ok_or_else(|| DecodeError::new(TARGET, MissingElement::new("d:multistatus")))?;

    let mut versions = Vec::new();
    let file_id_str = file_id.to_string();

    for response in multistatus.children("d:response") {
        let href = response
            .child("d:href")
            .map(|e| e.text())
            .filter(|h| !h.is_empty())
            .ok_or_else(|| DecodeError::new(TARGET, MissingElement::new("d:href")))?;

        let trimmed_href = href.trim_end_matches('/');
        let last_segment = trimmed_href.rsplit('/').next().unwrap_or("");

        // Skip the collection itself (which either ends in the file_id or has collection resourcetype)
        if last_segment == file_id_str || response.has("d:collection") {
            continue;
        }

        if last_segment.is_empty() {
            continue;
        }

        let revision_id = last_segment.to_owned();

        let props: Vec<_> = response
            .children("d:propstat")
            .into_iter()
            .filter(is_successful_propstat)
            .filter_map(|ps| ps.child("d:prop"))
            .collect();

        let last_modified = props
            .iter()
            .find_map(|p| p.child("d:getlastmodified").map(|e| e.text()));

        let content_length = props.iter().find_map(|p| {
            p.child("d:getcontentlength")
                .and_then(|e| e.text().trim().parse::<u64>().ok())
        });

        let etag = props
            .iter()
            .find_map(|p| p.child("d:getetag").map(|e| e.text()));

        versions.push(FileVersion::new(
            href,
            revision_id,
            last_modified,
            content_length,
            etag,
        ));
    }

    Ok(versions)
}

fn is_successful_propstat(propstat: &xml::Element<'_>) -> bool {
    match propstat.child("d:status") {
        None => true,
        Some(status) => status.text().contains(" 200 "),
    }
}

#[cfg(test)]
mod tests {
    use super::decode_multistatus;

    const SAMPLE_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/versions/alice/versions/12345/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/versions/alice/versions/12345/1609459200</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Fri, 01 Jan 2021 00:00:00 GMT</d:getlastmodified>
        <d:getcontentlength>1024</d:getcontentlength>
        <d:getetag>"rev1-etag"</d:getetag>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/versions/alice/versions/12345/1609545600</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Sat, 02 Jan 2021 00:00:00 GMT</d:getlastmodified>
        <d:getcontentlength>2048</d:getcontentlength>
        <d:getetag>"rev2-etag"</d:getetag>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn decodes_revisions_and_skips_parent_collection() {
        let versions = decode_multistatus(SAMPLE_MULTISTATUS, 12345).expect("valid xml");
        assert_eq!(versions.len(), 2);

        assert_eq!(versions[0].revision_id, "1609459200");
        assert_eq!(
            versions[0].href,
            "/remote.php/dav/versions/alice/versions/12345/1609459200"
        );
        assert_eq!(
            versions[0].last_modified.as_deref(),
            Some("Fri, 01 Jan 2021 00:00:00 GMT")
        );
        assert_eq!(versions[0].content_length, Some(1024));
        assert_eq!(versions[0].etag.as_deref(), Some("\"rev1-etag\""));

        assert_eq!(versions[1].revision_id, "1609545600");
        assert_eq!(
            versions[1].href,
            "/remote.php/dav/versions/alice/versions/12345/1609545600"
        );
        assert_eq!(
            versions[1].last_modified.as_deref(),
            Some("Sat, 02 Jan 2021 00:00:00 GMT")
        );
        assert_eq!(versions[1].content_length, Some(2048));
        assert_eq!(versions[1].etag.as_deref(), Some("\"rev2-etag\""));
    }

    #[test]
    fn empty_multistatus_returns_no_versions() {
        let empty = r#"<d:multistatus xmlns:d="DAV:"></d:multistatus>"#;
        let versions = decode_multistatus(empty, 12345).expect("valid empty xml");
        assert!(versions.is_empty());
    }

    #[test]
    fn invalid_xml_without_multistatus_fails() {
        let invalid = "<error>Not a multistatus</error>";
        assert!(decode_multistatus(invalid, 12345).is_err());
    }

    #[test]
    fn missing_href_fails_decode() {
        let missing_href = r#"<d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:propstat><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
          </d:response>
        </d:multistatus>"#;
        assert!(decode_multistatus(missing_href, 12345).is_err());
    }
}
