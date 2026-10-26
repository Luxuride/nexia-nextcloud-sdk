//! Embedded EXIF thumbnail extraction for JPEG images.

/// Extracts the raw bytes of an embedded JPEG thumbnail from image header bytes
/// (typically the first 64–128 KB of a camera JPEG containing EXIF metadata).
///
/// Returns `Some(&[u8])` with the complete slice from SOI (`0xFF, 0xD8`) to EOI (`0xFF, 0xD9`)
/// if an embedded thumbnail is found, or `None` if absent.
#[must_use]
pub fn extract_embedded_jpeg_thumbnail(header_bytes: &[u8]) -> Option<&[u8]> {
    if header_bytes.len() < 4 || header_bytes[0..2] != [0xFF, 0xD8] {
        return None;
    }

    // Secondary SOI marker search within EXIF boundaries (typically within first 64 KB)
    let max_scan = header_bytes.len().min(65536);
    for i in 2..max_scan.saturating_sub(1) {
        if header_bytes[i] == 0xFF
            && header_bytes[i + 1] == 0xD8
            && let Some(end_rel) = header_bytes[i..].windows(2).position(|w| w == [0xFF, 0xD9])
        {
            let end = i + end_rel + 2;
            if end > i + 4 {
                return Some(&header_bytes[i..end]);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_embedded_jpeg_when_present() {
        let mut data = Vec::new();
        // Main JPEG SOI + dummy APP1
        data.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x20]);
        data.extend_from_slice(b"Exif\x00\x00DummyHeaderContent");

        // Embedded thumbnail SOI + payload + EOI
        let thumb_start = data.len();
        data.extend_from_slice(&[0xFF, 0xD8, 0x01, 0x02, 0x03, 0xFF, 0xD9]);
        let thumb_end = data.len();

        // Remainder of main JPEG
        data.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x0C, 0xFF, 0xD9]);

        let extracted = extract_embedded_jpeg_thumbnail(&data);
        assert_eq!(extracted, Some(&data[thumb_start..thumb_end]));
    }

    #[test]
    fn returns_none_when_no_secondary_soi() {
        let data = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0xFF, 0xD9];
        assert_eq!(extract_embedded_jpeg_thumbnail(&data), None);
    }

    #[test]
    fn returns_none_for_non_jpeg() {
        let data = b"\x89PNG\r\n\x1a\n";
        assert_eq!(extract_embedded_jpeg_thumbnail(data), None);
    }
}
