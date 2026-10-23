use std::fmt;

/// HTTP method used for a request.
///
/// The WebDAV methods are part of the enum because Nextcloud's file API is
/// WebDAV: there is no way to reach it with the plain HTTP verbs alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    /// `GET`
    Get,
    /// `POST`
    Post,
    /// `PUT`
    Put,
    /// `PATCH`
    Patch,
    /// `DELETE`
    Delete,
    /// `PROPFIND`
    Propfind,
    /// `PROPPATCH`
    Proppatch,
    /// `MKCOL`
    Mkcol,
    /// `MOVE`
    Move,
    /// `COPY`
    Copy,
    /// `REPORT`
    Report,
}

impl HttpMethod {
    /// Wire representation of the method.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Propfind => "PROPFIND",
            Self::Proppatch => "PROPPATCH",
            Self::Mkcol => "MKCOL",
            Self::Move => "MOVE",
            Self::Copy => "COPY",
            Self::Report => "REPORT",
        }
    }

    /// Whether a request with this method is expected to carry a body.
    pub const fn expects_body(self) -> bool {
        matches!(
            self,
            Self::Post | Self::Put | Self::Patch | Self::Propfind | Self::Proppatch | Self::Mkcol
        )
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::HttpMethod;

    #[test]
    fn webdav_methods_use_upper_case_wire_names() {
        assert_eq!(HttpMethod::Propfind.as_str(), "PROPFIND");
        assert_eq!(HttpMethod::Mkcol.as_str(), "MKCOL");
        assert_eq!(HttpMethod::Report.to_string(), "REPORT");
    }

    #[test]
    fn get_is_not_expected_to_carry_a_body() {
        assert!(!HttpMethod::Get.expects_body());
        assert!(HttpMethod::Propfind.expects_body());
    }
}
