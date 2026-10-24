//! Nextcloud Activity API client and domain types.
//!
//! Nextcloud provides an activity stream tracking user events (file changes, shares, comments,
//! calendar updates, and third-party app actions) through its Activity OCS API v2 at
//! `/ocs/v2.php/apps/activity/api/v2/activity`.
//!
//! This crate provides typed models for activities, query filter builders, and an asynchronous
//! client for consuming the activity feed.
//!
//! - [`ActivityClient`] — fetches activity streams and queries activities for specific objects.
//! - [`Activity`] — represents an individual activity item.
//! - [`ActivityFilter`] — builds query parameters for pagination, sorting, and object filters.
//!
//! Official documentation:
//! - [Nextcloud Activity API Documentation](https://docs.nextcloud.com/server/latest/developer_manual/client_apis/activity-api.html)
//! - [Developer manual source](https://github.com/nextcloud/documentation/blob/master/developer_manual/client_apis/activity-api.rst)
//! - [Activity app endpoint specification](https://github.com/nextcloud/activity/blob/master/docs/endpoint-v2.md)
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod activity;
mod client;
mod query;
mod wire;

pub use activity::Activity;
pub use client::ActivityClient;
pub use query::ActivityFilter;
