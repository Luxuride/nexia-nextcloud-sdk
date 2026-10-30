//! Query strings for OCS calls, including the `format` selector.

mod encoding;
mod format;
mod query_string;

pub use format::OcsFormat;
pub use query_string::Query;
