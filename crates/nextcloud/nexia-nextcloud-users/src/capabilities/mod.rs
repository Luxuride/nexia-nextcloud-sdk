//! Server capabilities models and deserialization.
//!
//! Nextcloud reports server capabilities and installed feature sets via the
//! OCS capabilities API (`/ocs/v1.php/cloud/capabilities` or
//! `/ocs/v2.php/cloud/capabilities`).
//!
//! Official documentation:
//! <https://docs.nextcloud.com/server/latest/developer_manual/client_apis/OCS/ocs-api-overview.html#capabilities-api>

mod chunked_upload;
mod core;
mod files;
mod response;
mod status;
mod theming;
mod version;

#[cfg(test)]
mod tests;

pub use chunked_upload::ChunkedUpload;
pub use core::CoreCapabilities;
pub use files::FilesCapabilities;
pub use response::{Capabilities, CapabilitiesData};
pub use status::ServerStatus;
pub use theming::ThemingCapabilities;
pub use version::ServerVersion;
