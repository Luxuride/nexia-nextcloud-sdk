//! Query parameter encoding for Nextcloud preview requests.

use crate::options::PreviewOptions;

/// Encodes query parameters for a preview request by numeric file cache ID.
pub(crate) fn build_file_id_query(file_id: u64, options: &PreviewOptions) -> String {
    let mut parts = Vec::new();
    parts.push(format!("fileId={file_id}"));
    append_options(&mut parts, options);
    format!("?{}", parts.join("&"))
}

/// Encodes query parameters for a preview request by file path.
pub(crate) fn build_path_query(path: &str, options: &PreviewOptions) -> String {
    let mut parts = Vec::new();
    let normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    parts.push(format!("file={}", encode_path_component(&normalized)));
    append_options(&mut parts, options);
    format!("?{}", parts.join("&"))
}

fn append_options(parts: &mut Vec<String>, options: &PreviewOptions) {
    if let Some(x) = options.width_px() {
        parts.push(format!("x={x}"));
    }
    if let Some(y) = options.height_px() {
        parts.push(format!("y={y}"));
    }
    if let Some(a) = options.aspect_ratio() {
        let val = if a { "1" } else { "0" };
        parts.push(format!("a={val}"));
    }
    if let Some(mode) = options.preview_mode() {
        parts.push(format!("mode={}", mode.as_str()));
    }
}

/// Percent-encodes a URI path query value following RFC 3986, preserving `/` path delimiters.
fn encode_path_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            other => {
                encoded.push('%');
                encoded.push(hex_digit(other >> 4));
                encoded.push(hex_digit(other & 0x0f));
            }
        }
    }
    encoded
}

const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::PreviewMode;

    #[test]
    fn file_id_query_minimal() {
        let options = PreviewOptions::new();
        assert_eq!(build_file_id_query(42, &options), "?fileId=42");
    }

    #[test]
    fn file_id_query_all_options() {
        let options = PreviewOptions::new()
            .width(64)
            .height(128)
            .keep_aspect_ratio(true)
            .mode(PreviewMode::Cover);

        assert_eq!(
            build_file_id_query(999, &options),
            "?fileId=999&x=64&y=128&a=1&mode=cover"
        );
    }

    #[test]
    fn path_query_minimal() {
        let options = PreviewOptions::new();
        assert_eq!(
            build_path_query("Documents/test.png", &options),
            "?file=/Documents/test.png"
        );
    }

    #[test]
    fn path_query_all_options() {
        let options = PreviewOptions::new()
            .width(200)
            .height(200)
            .keep_aspect_ratio(false)
            .mode(PreviewMode::Fill);

        assert_eq!(
            build_path_query("/Photos/photo 1.jpg", &options),
            "?file=/Photos/photo%201.jpg&x=200&y=200&a=0&mode=fill"
        );
    }

    #[test]
    fn encode_path_component_escapes_special_characters() {
        assert_eq!(encode_path_component("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(encode_path_component("hello/world"), "hello/world");
        assert_eq!(
            encode_path_component("simple-name_1.txt~"),
            "simple-name_1.txt~"
        );
    }
}
