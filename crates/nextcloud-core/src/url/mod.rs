//! Nextcloud base URLs and the request paths beneath them.

mod api_path;
mod error;
mod server_url;

pub use api_path::ApiPath;
pub use error::UrlError;
pub use server_url::ServerUrl;
