use sdk_core::{ApiPath, Error, UrlError};

use crate::query::Query;

/// Appends `query` to `path`, preserving the [`ApiPath`] variant.
///
/// `sdk_core::Request` carries a path but no separate query slot, and an
/// OCS endpoint such as `/ocs/v2.php/cloud/capabilities` only accepts its
/// parameters as a query string. The assembled path therefore carries the
/// query verbatim and reaches the transport as one unit, which is what the
/// server sees on the wire.
pub(crate) fn with_query(path: &ApiPath, query: &Query) -> ApiPath {
    let suffix = query.build();
    match path {
        ApiPath::Ocs(base) => ApiPath::Ocs(format!("{base}{suffix}")),
        ApiPath::Dav(base) => ApiPath::Dav(format!("{base}{suffix}")),
        ApiPath::Public(base) => ApiPath::Public(format!("{base}{suffix}")),
    }
}

/// Rejects a non-OCS path instead of sending it to an OCS endpoint.
///
/// `/ocs/v2.php` is the only root that answers with the envelope this crate
/// decodes, so a WebDAV or public path is reported before a request is made
/// rather than decoded into an empty payload.
pub(crate) fn require_ocs(path: ApiPath) -> Result<ApiPath, Error> {
    match path {
        ApiPath::Ocs(_) => Ok(path),
        other => Err(Error::Url(UrlError::RelativePath(
            other.as_str().to_owned(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use sdk_core::ApiPath;

    use super::{require_ocs, with_query};
    use crate::query::{OcsFormat, Query};

    #[test]
    fn query_is_appended_to_the_ocs_path() {
        let path = ApiPath::ocs("cloud/capabilities").expect("valid OCS path");
        let query = Query::new().format(OcsFormat::Json);
        assert_eq!(
            with_query(&path, &query).as_str(),
            "/ocs/v2.php/cloud/capabilities?format=json"
        );
    }

    #[test]
    fn empty_query_leaves_the_path_untouched() {
        let path = ApiPath::ocs("cloud/user").expect("valid OCS path");
        assert_eq!(with_query(&path, &Query::new()).as_str(), path.as_str());
    }

    #[test]
    fn ocs_paths_are_accepted() {
        let path = ApiPath::ocs("cloud/user").expect("valid OCS path");
        let accepted = require_ocs(path.clone()).expect("an OCS path is accepted");
        assert_eq!(accepted, path);
    }

    #[test]
    fn non_ocs_paths_are_rejected() {
        let error = require_ocs(ApiPath::dav("files/user").expect("valid DAV path"))
            .expect_err("a DAV path is not an OCS call");
        assert!(error.as_url().is_some(), "rejected as a URL before sending");
    }
}
