//! HTTP basic authentication with a Nextcloud app password.

mod app_password;
mod basic_auth;

pub use app_password::AppPassword;
pub use basic_auth::BasicAuth;
