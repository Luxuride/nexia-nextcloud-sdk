//! HTTP vocabulary shared by every API area.

mod header;
mod headers;
mod method;
mod status_code;

pub use header::Header;
pub use headers::Headers;
pub use method::HttpMethod;
pub use status_code::StatusCode;
