//! Nextcloud Preview and Thumbnail API client and domain types.
//!
//! Nextcloud provides an endpoint for generating on-demand file previews and thumbnails
//! at `/core/preview.png` (routed via `/index.php/core/preview.png`).
//!
//! - [`PreviewClient`] — asynchronous client for fetching image previews.
//! - [`PreviewOptions`] — builder for dimension, aspect ratio, and mode parameters.
//! - [`PreviewMode`] — scaling mode (`Cover` or `Fill`).
//!
//! Official Nextcloud Preview endpoint:
//! `GET /core/preview.png?fileId={fileId}&x={x}&y={y}&a={a}&mode={mode}` or
//! `GET /core/preview.png?file={filePath}&x={x}&y={y}`.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod client;
pub mod exif;
pub mod freedesktop;
mod mode;
mod options;
mod query;

pub use client::{PREVIEW_ENDPOINT, PreviewClient};
pub use exif::extract_embedded_jpeg_thumbnail;
pub use freedesktop::{
    crc32, embed_freedesktop_metadata, ensure_png_bytes, freedesktop_thumbnail_dirs,
    freedesktop_thumbnail_is_current, install_freedesktop_fail_marker,
    install_freedesktop_thumbnail, install_freedesktop_thumbnail_with_mtime, make_png_text_chunk,
    md5_hex, path_to_file_uri, remove_freedesktop_fail_marker,
};
pub use mode::PreviewMode;
pub use options::PreviewOptions;
