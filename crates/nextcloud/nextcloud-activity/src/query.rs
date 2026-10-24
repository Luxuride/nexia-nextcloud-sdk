//! Query filter builder for Nextcloud Activity API requests.
//!
//! Official documentation:
//! - [Activity API Parameters](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#parameters)
//! - [Developer manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst)

use nextcloud_ocs::{OcsFormat, Query};

/// Query builder for filtering activities in the activity stream.
///
/// Corresponds to the query parameters accepted by
/// `GET /ocs/v2.php/apps/activity/api/v2/activity`.
///
/// See the [Nextcloud Activity API documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html#parameters)
/// and [developer_manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivityFilter {
    /// ID of the last activity seen (used for pagination to fetch entries relative to this ID).
    pub since: Option<u64>,

    /// Maximum number of activities to return per page (server default is 50).
    pub limit: Option<u32>,

    /// Category or type of the target object to filter by (e.g. `"files"`).
    pub object_type: Option<String>,

    /// Numeric identifier of the target object to filter by.
    pub object_id: Option<u64>,

    /// Sort direction relative to `since`: `"asc"` or `"desc"` (server default is `"desc"`).
    pub sort: Option<String>,
}

impl ActivityFilter {
    /// Creates a new empty `ActivityFilter`.
    pub const fn new() -> Self {
        Self {
            since: None,
            limit: None,
            object_type: None,
            object_id: None,
            sort: None,
        }
    }

    /// Sets the `since` activity ID threshold for pagination.
    #[must_use]
    pub const fn since(mut self, since: u64) -> Self {
        self.since = Some(since);
        self
    }

    /// Sets the maximum number of activities to return.
    #[must_use]
    pub const fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the object type filter (e.g. `"files"`).
    #[must_use]
    pub fn object_type(mut self, object_type: impl Into<String>) -> Self {
        self.object_type = Some(object_type.into());
        self
    }

    /// Sets the object ID filter.
    #[must_use]
    pub const fn object_id(mut self, object_id: u64) -> Self {
        self.object_id = Some(object_id);
        self
    }

    /// Convenience helper to set both `object_type` and `object_id` together.
    #[must_use]
    pub fn object(mut self, object_type: impl Into<String>, object_id: u64) -> Self {
        self.object_type = Some(object_type.into());
        self.object_id = Some(object_id);
        self
    }

    /// Sets the sort direction (`"asc"` or `"desc"`).
    #[must_use]
    pub fn sort(mut self, sort: impl Into<String>) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Converts this filter into an OCS [`Query`] with `format=json` appended.
    pub fn to_query(&self) -> Query {
        let mut query = Query::new().format(OcsFormat::Json);

        if let Some(since) = self.since {
            query = query.param("since", since.to_string());
        }
        if let Some(limit) = self.limit {
            query = query.param("limit", limit.to_string());
        }
        if let Some(ref object_type) = self.object_type {
            query = query.param("object_type", object_type);
        }
        if let Some(object_id) = self.object_id {
            query = query.param("object_id", object_id.to_string());
        }
        if let Some(ref sort) = self.sort {
            query = query.param("sort", sort);
        }

        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_filter_produces_format_json() {
        let filter = ActivityFilter::new();
        let query = filter.to_query();

        let params: Vec<(&str, &str)> = query.iter().collect();
        assert_eq!(params, vec![("format", "json")]);
    }

    #[test]
    fn full_filter_builds_all_parameters() {
        let filter = ActivityFilter::new()
            .since(123)
            .limit(25)
            .object_type("files")
            .object_id(456)
            .sort("asc");

        let query = filter.to_query();
        let params: Vec<(&str, &str)> = query.iter().collect();

        assert_eq!(
            params,
            vec![
                ("format", "json"),
                ("since", "123"),
                ("limit", "25"),
                ("object_type", "files"),
                ("object_id", "456"),
                ("sort", "asc"),
            ]
        );
    }

    #[test]
    fn object_convenience_method_sets_both_fields() {
        let filter = ActivityFilter::new().object("shares", 789);
        assert_eq!(filter.object_type.as_deref(), Some("shares"));
        assert_eq!(filter.object_id, Some(789));

        let query = filter.to_query();
        let params: Vec<(&str, &str)> = query.iter().collect();
        assert!(params.contains(&("object_type", "shares")));
        assert!(params.contains(&("object_id", "789")));
    }
}
