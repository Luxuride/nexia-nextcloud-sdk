//! The `<d:propfind>` document a `PROPFIND` request carries.

use crate::xml::XmlWriter;

use super::{DavNamespace, DavProperty};

/// A `PROPFIND` request body asking for a set of properties.
///
/// Namespace declarations are emitted here rather than by the writer, because
/// they belong to this document shape: `oc` and `nc` are only needed when a
/// property from that namespace is asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropfindBody {
    properties: Vec<DavProperty>,
}

impl PropfindBody {
    /// Asks for exactly `properties`.
    pub fn new(properties: impl IntoIterator<Item = DavProperty>) -> Self {
        Self {
            properties: properties.into_iter().collect(),
        }
    }

    /// Asks for every property this crate decodes.
    pub fn all() -> Self {
        Self::new([
            DavProperty::Etag,
            DavProperty::ContentLength,
            DavProperty::ContentType,
            DavProperty::ResourceType,
            DavProperty::LastModified,
            DavProperty::DisplayName,
            DavProperty::FileId,
            DavProperty::Permissions,
            DavProperty::Size,
            DavProperty::HasPreview,
        ])
    }

    /// Properties this body asks for.
    pub fn properties(&self) -> &[DavProperty] {
        &self.properties
    }

    /// Whether nothing is asked for, in which case the request would be
    /// pointless.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Renders the document, namespace declarations included.
    pub fn to_xml(&self) -> String {
        let mut writer = XmlWriter::new();
        writer.prolog();
        writer.open_with("d:propfind", &self.namespace_declarations());
        writer.open("d:prop");
        for property in &self.properties {
            writer.empty(property.qualified_name());
        }
        writer.close("d:prop");
        writer.close("d:propfind");
        writer.into_string()
    }

    /// `DAV:` is always declared; the Nextcloud namespaces only when used, so
    /// a request for core properties stays free of unrelated declarations.
    fn namespace_declarations(&self) -> Vec<(&'static str, &'static str)> {
        let mut declarations: Vec<(&'static str, &'static str)> = Vec::new();
        for namespace in self.properties.iter().map(|p| p.namespace()) {
            if declarations
                .iter()
                .any(|(key, _)| *key == declaration_key(namespace))
            {
                continue;
            }
            declarations.push((declaration_key(namespace), namespace.uri()));
        }
        if declarations.is_empty() {
            let namespace = DavNamespace::Dav;
            declarations.push((declaration_key(namespace), namespace.uri()));
        }
        declarations
    }
}

/// Attribute name declaring `namespace`'s prefix.
const fn declaration_key(namespace: DavNamespace) -> &'static str {
    match namespace {
        DavNamespace::Dav => "xmlns:d",
        DavNamespace::OwnCloud => "xmlns:oc",
        DavNamespace::Nextcloud => "xmlns:nc",
    }
}

#[cfg(test)]
mod tests {
    use super::PropfindBody;
    use crate::propfind::DavProperty;

    #[test]
    fn every_preset_contains_the_three_namespace_declarations() {
        let xml = PropfindBody::all().to_xml();
        assert!(xml.contains(r#"xmlns:d="DAV:""#), "{xml}");
        assert!(
            xml.contains(r#"xmlns:oc="http://owncloud.org/ns""#),
            "{xml}"
        );
        assert!(
            xml.contains(r#"xmlns:nc="http://nextcloud.org/ns""#),
            "{xml}"
        );
    }

    #[test]
    fn preset_asks_for_every_property_once() {
        let xml = PropfindBody::all().to_xml();
        assert_eq!(xml.matches("<d:getetag/>").count(), 1);
        assert!(xml.contains("<oc:fileid/>"));
        assert!(xml.contains("<nc:has-preview/>"));
        assert_eq!(xml.matches("xmlns:d=").count(), 1);
    }

    #[test]
    fn body_is_well_formed_and_carries_the_requested_properties() {
        let xml = PropfindBody::new([DavProperty::Etag, DavProperty::FileId]).to_xml();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(xml.contains("<d:propfind"));
        assert!(xml.contains("<d:prop><d:getetag/><oc:fileid/></d:prop>"));
        assert!(xml.ends_with("</d:propfind>"));
        assert_eq!(xml.matches("<d:prop>").count(), 1);
        assert_eq!(xml.matches("</d:propfind>").count(), 1);
        assert!(!xml.contains("xmlns:nc"));
    }

    #[test]
    fn core_only_body_still_declares_the_dav_namespace() {
        let xml = PropfindBody::new([DavProperty::Etag]).to_xml();
        assert!(xml.contains(r#"xmlns:d="DAV:""#));
        assert!(!xml.contains("xmlns:oc"));
    }

    #[test]
    fn empty_body_is_reported_and_still_renders_a_document() {
        let body = PropfindBody::new([]);
        assert!(body.is_empty());
        assert!(body.to_xml().contains("<d:prop></d:prop>"));
    }
}
