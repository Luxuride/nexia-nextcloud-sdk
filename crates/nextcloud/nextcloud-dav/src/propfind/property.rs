//! The properties this crate knows how to read back.

/// XML namespace a WebDAV property is defined in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DavNamespace {
    /// `DAV:` — the WebDAV core namespace, prefix `d`.
    Dav,
    /// `http://owncloud.org/ns` — the legacy ownCloud namespace, prefix `oc`.
    OwnCloud,
    /// `http://nextcloud.org/ns` — the Nextcloud namespace, prefix `nc`.
    Nextcloud,
}

impl DavNamespace {
    /// Prefix used for this namespace in request bodies.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Dav => "d",
            Self::OwnCloud => "oc",
            Self::Nextcloud => "nc",
        }
    }

    /// Namespace URI as declared on the propfind element.
    pub const fn uri(self) -> &'static str {
        match self {
            Self::Dav => "DAV:",
            Self::OwnCloud => "http://owncloud.org/ns",
            Self::Nextcloud => "http://nextcloud.org/ns",
        }
    }
}

/// A property a propfind request may ask for.
///
/// The set is deliberately the one [`DavResource`](crate::DavResource) can
/// represent: asking for a property nothing decodes would only cost bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DavProperty {
    /// `d:getetag` — entity tag of the current content.
    Etag,
    /// `d:getcontentlength` — size in bytes.
    ContentLength,
    /// `d:getcontenttype` — MIME type.
    ContentType,
    /// `d:resourcetype` — collection marker.
    ResourceType,
    /// `d:getlastmodified` — modification time, as an HTTP date.
    LastModified,
    /// `d:displayname` — user-visible name.
    DisplayName,
    /// `oc:fileid` — Nextcloud's stable file id.
    FileId,
    /// `oc:permissions` — permission letters for this resource.
    Permissions,
    /// `nc:has-preview` — whether a preview can be rendered.
    HasPreview,
}

impl DavProperty {
    /// Namespace the property is defined in.
    pub const fn namespace(self) -> DavNamespace {
        match self {
            Self::Etag
            | Self::ContentLength
            | Self::ContentType
            | Self::ResourceType
            | Self::LastModified
            | Self::DisplayName => DavNamespace::Dav,
            Self::FileId | Self::Permissions => DavNamespace::OwnCloud,
            Self::HasPreview => DavNamespace::Nextcloud,
        }
    }

    /// Local part of the property name, without prefix.
    pub const fn local_name(self) -> &'static str {
        match self {
            Self::Etag => "getetag",
            Self::ContentLength => "getcontentlength",
            Self::ContentType => "getcontenttype",
            Self::ResourceType => "resourcetype",
            Self::LastModified => "getlastmodified",
            Self::DisplayName => "displayname",
            Self::FileId => "fileid",
            Self::Permissions => "permissions",
            Self::HasPreview => "has-preview",
        }
    }

    /// Qualified name used in both request and response XML.
    ///
    /// Spelled out rather than assembled from
    /// [`namespace`](Self::namespace) and [`local_name`](Self::local_name)
    /// because it has to stay a `&'static str`; the unit tests assert the
    /// three stay consistent.
    pub const fn qualified_name(self) -> &'static str {
        match self {
            Self::Etag => "d:getetag",
            Self::ContentLength => "d:getcontentlength",
            Self::ContentType => "d:getcontenttype",
            Self::ResourceType => "d:resourcetype",
            Self::LastModified => "d:getlastmodified",
            Self::DisplayName => "d:displayname",
            Self::FileId => "oc:fileid",
            Self::Permissions => "oc:permissions",
            Self::HasPreview => "nc:has-preview",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DavNamespace, DavProperty};

    const ALL: [DavProperty; 9] = [
        DavProperty::Etag,
        DavProperty::ContentLength,
        DavProperty::ContentType,
        DavProperty::ResourceType,
        DavProperty::LastModified,
        DavProperty::DisplayName,
        DavProperty::FileId,
        DavProperty::Permissions,
        DavProperty::HasPreview,
    ];

    #[test]
    fn every_property_renders_its_namespaced_tag() {
        let expected = [
            "d:getetag",
            "d:getcontentlength",
            "d:getcontenttype",
            "d:resourcetype",
            "d:getlastmodified",
            "d:displayname",
            "oc:fileid",
            "oc:permissions",
            "nc:has-preview",
        ];
        for (property, tag) in ALL.iter().zip(expected) {
            assert_eq!(property.qualified_name(), tag);
            let prefix = property.namespace().prefix();
            assert_eq!(
                property.qualified_name(),
                format!("{prefix}:{}", property.local_name())
            );
        }
    }

    #[test]
    fn namespaces_carry_the_prefix_and_uri_they_are_declared_with() {
        assert_eq!(DavNamespace::Dav.prefix(), "d");
        assert_eq!(DavNamespace::Dav.uri(), "DAV:");
        assert_eq!(DavNamespace::OwnCloud.uri(), "http://owncloud.org/ns");
        assert_eq!(DavNamespace::Nextcloud.uri(), "http://nextcloud.org/ns");
    }

    #[test]
    fn request_tags_match_the_response_tags() {
        assert_eq!(DavProperty::HasPreview.qualified_name(), "nc:has-preview");
        assert_eq!(DavProperty::FileId.qualified_name(), "oc:fileid");
    }
}
