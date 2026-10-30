//! Resource icon categorization and human-readable byte size formatting for VFS nodes.

use serde::{Deserialize, Serialize};

use crate::metadata::NodeMetadata;

/// Semantic icon category for a virtual file or directory following Freedesktop icon naming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceIconCategory {
    /// Generic directory folder.
    Folder,
    /// Documents directory folder.
    DocumentsFolder,
    /// Photos / Pictures directory folder.
    PicturesFolder,
    /// Music / Audio directory folder.
    MusicFolder,
    /// Videos / Movies directory folder.
    VideosFolder,
    /// Downloads directory folder.
    DownloadsFolder,
    /// Templates directory folder.
    TemplatesFolder,
    /// Public / Shared directory folder.
    PublicShareFolder,
    /// Office document or PDF.
    Document,
    /// Office spreadsheet or tabular data.
    Spreadsheet,
    /// Office presentation deck.
    Presentation,
    /// Image or graphic file.
    Image,
    /// Audio file.
    Audio,
    /// Video file.
    Video,
    /// Compressed archive or package.
    Archive,
    /// Plain text, markdown, or source code file.
    Text,
}

impl ResourceIconCategory {
    /// Returns the standard Freedesktop full-color icon name (used by Nautilus / Adwaita).
    pub const fn freedesktop_icon_name(&self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::DocumentsFolder => "folder-documents",
            Self::PicturesFolder => "folder-pictures",
            Self::MusicFolder => "folder-music",
            Self::VideosFolder => "folder-videos",
            Self::DownloadsFolder => "folder-download",
            Self::TemplatesFolder => "folder-templates",
            Self::PublicShareFolder => "folder-publicshare",
            Self::Document => "x-office-document",
            Self::Spreadsheet => "x-office-spreadsheet",
            Self::Presentation => "x-office-presentation",
            Self::Image => "image-x-generic",
            Self::Audio => "audio-x-generic",
            Self::Video => "video-x-generic",
            Self::Archive => "package-x-generic",
            Self::Text => "text-x-generic",
        }
    }

    /// Returns the standard Freedesktop symbolic icon name.
    pub const fn symbolic_icon_name(&self) -> &'static str {
        match self {
            Self::Folder => "folder-symbolic",
            Self::DocumentsFolder => "folder-documents-symbolic",
            Self::PicturesFolder => "folder-pictures-symbolic",
            Self::MusicFolder => "folder-music-symbolic",
            Self::VideosFolder => "folder-videos-symbolic",
            Self::DownloadsFolder => "folder-download-symbolic",
            Self::TemplatesFolder => "folder-templates-symbolic",
            Self::PublicShareFolder => "folder-publicshare-symbolic",
            Self::Document => "x-office-document-symbolic",
            Self::Spreadsheet => "x-office-spreadsheet-symbolic",
            Self::Presentation => "x-office-presentation-symbolic",
            Self::Image => "image-x-generic-symbolic",
            Self::Audio => "audio-x-generic-symbolic",
            Self::Video => "video-x-generic-symbolic",
            Self::Archive => "package-x-generic-symbolic",
            Self::Text => "text-x-generic-symbolic",
        }
    }
}

impl NodeMetadata {
    /// Classifies this node into a [`ResourceIconCategory`] based on directory name, MIME type, or extension.
    pub fn icon_category(&self) -> ResourceIconCategory {
        if self.is_directory {
            return match self.name.to_lowercase().as_str() {
                "documents" => ResourceIconCategory::DocumentsFolder,
                "photos" | "pictures" | "images" => ResourceIconCategory::PicturesFolder,
                "music" | "audio" => ResourceIconCategory::MusicFolder,
                "videos" | "movies" => ResourceIconCategory::VideosFolder,
                "downloads" => ResourceIconCategory::DownloadsFolder,
                "templates" => ResourceIconCategory::TemplatesFolder,
                "public" | "shared" => ResourceIconCategory::PublicShareFolder,
                _ => ResourceIconCategory::Folder,
            };
        }

        if let Some(ref mime) = self.mime_type {
            if mime.starts_with("image/") {
                return ResourceIconCategory::Image;
            } else if mime.starts_with("audio/") {
                return ResourceIconCategory::Audio;
            } else if mime.starts_with("video/") {
                return ResourceIconCategory::Video;
            } else if mime.contains("spreadsheet") || mime == "text/csv" {
                return ResourceIconCategory::Spreadsheet;
            } else if mime.contains("presentation") {
                return ResourceIconCategory::Presentation;
            } else if mime == "application/pdf" || mime.contains("opendocument.text") {
                return ResourceIconCategory::Document;
            } else if mime.contains("zip") || mime.contains("tar") || mime.contains("compressed") {
                return ResourceIconCategory::Archive;
            }
        }

        let lower = self.name.to_lowercase();
        if lower.ends_with(".pdf")
            || lower.ends_with(".odt")
            || lower.ends_with(".doc")
            || lower.ends_with(".docx")
        {
            ResourceIconCategory::Document
        } else if lower.ends_with(".ods") || lower.ends_with(".xlsx") || lower.ends_with(".csv") {
            ResourceIconCategory::Spreadsheet
        } else if lower.ends_with(".odp") || lower.ends_with(".pptx") {
            ResourceIconCategory::Presentation
        } else if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
            || lower.ends_with(".svg")
        {
            ResourceIconCategory::Image
        } else if lower.ends_with(".flac")
            || lower.ends_with(".mp3")
            || lower.ends_with(".ogg")
            || lower.ends_with(".wav")
        {
            ResourceIconCategory::Audio
        } else if lower.ends_with(".mp4") || lower.ends_with(".mkv") || lower.ends_with(".webm") {
            ResourceIconCategory::Video
        } else if lower.ends_with(".zip")
            || lower.ends_with(".tar")
            || lower.ends_with(".gz")
            || lower.ends_with(".7z")
        {
            ResourceIconCategory::Archive
        } else {
            ResourceIconCategory::Text
        }
    }
}

/// Formats a raw byte count into human-readable binary units (`B`, `KB`, `MB`, `GB`).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_categorization_for_folders_and_files() {
        let docs = NodeMetadata::directory("/Documents");
        assert_eq!(
            docs.icon_category().freedesktop_icon_name(),
            "folder-documents"
        );

        let pdf = NodeMetadata::file("/Manual.pdf").with_mime_type("application/pdf");
        assert_eq!(
            pdf.icon_category().freedesktop_icon_name(),
            "x-office-document"
        );

        let img = NodeMetadata::file("/Photos/sunset.jpg");
        assert_eq!(
            img.icon_category().freedesktop_icon_name(),
            "image-x-generic"
        );
    }

    #[test]
    fn byte_formatting() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }
}
