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
pub mod freedesktop;
mod mode;
mod options;
mod query;

pub use client::{PREVIEW_ENDPOINT, PreviewClient};
pub use freedesktop::{
    crc32, embed_freedesktop_metadata, ensure_png_bytes, freedesktop_thumbnail_dirs,
    install_freedesktop_thumbnail, install_freedesktop_thumbnail_with_mtime, make_png_text_chunk,
    md5_hex, path_to_file_uri,
};
pub use mode::PreviewMode;
pub use options::PreviewOptions;
