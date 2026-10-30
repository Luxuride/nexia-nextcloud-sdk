//! Nextcloud base URLs and the request paths beneath them.

mod api_path;
mod error;
mod percent;
mod server_url;

pub use api_path::ApiPath;
pub use error::UrlError;
pub use percent::{percent_encode, percent_encode_path};
pub use server_url::ServerUrl;
