//! PROPFIND request building and multistatus XML decoding for trashbin items.

use nextcloud_dav::xml::response::{first_element, has_element, text_of};
use nextcloud_dav::xml::{Element, MissingElement, XmlWriter};
use sdk_core::DecodeError;

use crate::item::TrashItem;

/// Target name used in [`DecodeError`]s emitted by this module.
pub(crate) const TARGET: &str = "TrashItem";

/// Assembles the XML request body for the trashbin `PROPFIND` query.
///
/// Requests:
/// - `d:getlastmodified`
/// - `d:resourcetype`
/// - `oc:trashbin-filename` and `nc:trashbin-filename`
/// - `oc:trashbin-original-location` and `nc:trashbin-original-location`
/// - `oc:trashbin-deletion-time` and `nc:trashbin-deletion-time`
pub(crate) fn build_propfind_body() -> String {
    let mut writer = XmlWriter::new();
    writer.prolog();
    writer.open_with(
        "d:propfind",
        &[
            ("xmlns:d", "DAV:"),
            ("xmlns:oc", "http://owncloud.org/ns"),
            ("xmlns:nc", "http://nextcloud.org/ns"),
        ],
    );
    writer.open("d:prop");
    writer.empty("d:getlastmodified");
    writer.empty("d:resourcetype");
    writer.empty("oc:trashbin-filename");
    writer.empty("oc:trashbin-original-location");
    writer.empty("oc:trashbin-deletion-time");
    writer.empty("nc:trashbin-filename");
    writer.empty("nc:trashbin-original-location");
    writer.empty("nc:trashbin-deletion-time");
    writer.close("d:prop");
    writer.close("d:propfind");
    writer.into_string()
}

/// Decodes a `207 Multi-Status` XML response into a list of [`TrashItem`]s.
///
/// The trashbin root collection itself is omitted from the result.
pub(crate) fn decode_multistatus(user_id: &str, body: &str) -> Result<Vec<TrashItem>, DecodeError> {
    let multistatus = first_element(body, "d:multistatus")
        .or_else(|| first_element(body, "D:multistatus"))
        .ok_or_else(|| DecodeError::new(TARGET, MissingElement::new("d:multistatus")))?;

    let mut responses = multistatus.children("d:response");
    if responses.is_empty() {
        responses = multistatus.children("D:response");
    }

    let mut items = Vec::new();
    for response in responses {
        if let Some(item) = decode_response(user_id, &response)? {
            items.push(item);
        }
    }

    Ok(items)
}

fn decode_response(
    user_id: &str,
    response: &Element<'_>,
) -> Result<Option<TrashItem>, DecodeError> {
    let href = response
        .child("d:href")
        .or_else(|| response.child("D:href"))
        .map(|element| element.text())
        .filter(|href| !href.is_empty())
        .ok_or_else(|| DecodeError::new(TARGET, MissingElement::new("d:href")))?;

    // Skip the trash root collection itself.
    if is_trash_root(user_id, &href) {
        return Ok(None);
    }

    let propstats = {
        let mut ps = response.children("d:propstat");
        if ps.is_empty() {
            ps = response.children("D:propstat");
        }
        ps
    };

    let props: Vec<Element<'_>> = propstats
        .into_iter()
        .filter(is_successful_propstat)
        .filter_map(|propstat| {
            propstat
                .child("d:prop")
                .or_else(|| propstat.child("D:prop"))
        })
        .collect();

    let filename = find_property(
        &props,
        &[
            "oc:trashbin-filename",
            "nc:trashbin-filename",
            "trashbin-filename",
        ],
    );

    let original_location = find_property(
        &props,
        &[
            "oc:trashbin-original-location",
            "nc:trashbin-original-location",
            "trashbin-original-location",
        ],
    );

    let deletion_time = find_property(
        &props,
        &[
            "oc:trashbin-deletion-time",
            "nc:trashbin-deletion-time",
            "trashbin-deletion-time",
        ],
    )
    .and_then(|value| value.trim().parse::<u64>().ok());

    let last_modified = find_property(
        &props,
        &["d:getlastmodified", "D:getlastmodified", "getlastmodified"],
    );

    let is_collection = props.iter().any(|prop| {
        has_element(prop.inner(), "d:collection")
            || has_element(prop.inner(), "D:collection")
            || has_element(prop.inner(), "collection")
    });

    let trimmed_href = href.trim_end_matches('/');
    let id = trimmed_href.rsplit('/').next().unwrap_or("").to_owned();

    // If there is no filename and no deletion time and id matches the root, skip.
    if filename.is_none() && deletion_time.is_none() && id == "trash" {
        return Ok(None);
    }

    let filename = filename.unwrap_or_else(|| id.clone());
    let original_location = original_location.unwrap_or_else(|| filename.clone());

    Ok(Some(TrashItem {
        id,
        href,
        filename,
        original_location,
        deletion_time,
        last_modified,
        is_collection,
    }))
}

fn is_trash_root(user_id: &str, href: &str) -> bool {
    let trimmed = href.trim_end_matches('/');
    let expected_suffix = format!("/trashbin/{user_id}/trash");
    trimmed.ends_with(&expected_suffix)
}

fn is_successful_propstat(propstat: &Element<'_>) -> bool {
    let status_child = propstat
        .child("d:status")
        .or_else(|| propstat.child("D:status"));
    match status_child {
        None => true,
        Some(status) => status.text().contains(" 200 "),
    }
}

fn find_property(props: &[Element<'_>], names: &[&str]) -> Option<String> {
    for prop in props {
        for &name in names {
            if let Some(text) = text_of(prop.inner(), name) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRASH_MULTISTATUS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/notes.txt.d1600000000</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Wed, 21 Oct 2020 07:28:00 GMT</d:getlastmodified>
        <d:resourcetype/>
        <oc:trashbin-filename>notes.txt</oc:trashbin-filename>
        <oc:trashbin-original-location>Documents/notes.txt</oc:trashbin-original-location>
        <oc:trashbin-deletion-time>1600000000</oc:trashbin-deletion-time>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/Holiday.d1600000100/</d:href>
    <d:propstat>
      <d:prop>
        <d:getlastmodified>Wed, 21 Oct 2020 07:30:00 GMT</d:getlastmodified>
        <d:resourcetype><d:collection/></d:resourcetype>
        <nc:trashbin-filename>Holiday</nc:trashbin-filename>
        <nc:trashbin-original-location>Photos/Holiday</nc:trashbin-original-location>
        <nc:trashbin-deletion-time>1600000100</nc:trashbin-deletion-time>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn decodes_items_and_filters_root() {
        let items = decode_multistatus("alice", TRASH_MULTISTATUS).expect("decodes");
        assert_eq!(items.len(), 2);

        let file = &items[0];
        assert_eq!(file.id, "notes.txt.d1600000000");
        assert_eq!(file.filename, "notes.txt");
        assert_eq!(file.original_location, "Documents/notes.txt");
        assert_eq!(file.deletion_time, Some(1600000000));
        assert_eq!(
            file.last_modified.as_deref(),
            Some("Wed, 21 Oct 2020 07:28:00 GMT")
        );
        assert!(!file.is_collection);

        let folder = &items[1];
        assert_eq!(folder.id, "Holiday.d1600000100");
        assert_eq!(folder.filename, "Holiday");
        assert_eq!(folder.original_location, "Photos/Holiday");
        assert_eq!(folder.deletion_time, Some(1600000100));
        assert!(folder.is_collection);
    }

    #[test]
    fn decodes_empty_trash() {
        let empty_body = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/trashbin/alice/trash/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let items = decode_multistatus("alice", empty_body).expect("decodes");
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn propfind_body_contains_required_namespaces_and_properties() {
        let body = build_propfind_body();
        assert!(body.contains("xmlns:d=\"DAV:\""));
        assert!(body.contains("xmlns:oc=\"http://owncloud.org/ns\""));
        assert!(body.contains("xmlns:nc=\"http://nextcloud.org/ns\""));
        assert!(body.contains("<d:getlastmodified/>"));
        assert!(body.contains("<d:resourcetype/>"));
        assert!(body.contains("<oc:trashbin-filename/>"));
        assert!(body.contains("<oc:trashbin-original-location/>"));
        assert!(body.contains("<oc:trashbin-deletion-time/>"));
    }

    #[test]
    fn rejects_missing_multistatus() {
        let err = decode_multistatus("alice", "<html>Not found</html>").unwrap_err();
        assert_eq!(err.target(), TARGET);
    }

    #[test]
    fn rejects_missing_href() {
        let bad_xml = r#"<d:multistatus xmlns:d="DAV:"><d:response>
            <d:propstat><d:prop/></d:propstat>
        </d:response></d:multistatus>"#;
        let err = decode_multistatus("alice", bad_xml).unwrap_err();
        assert_eq!(err.target(), TARGET);
    }
}
