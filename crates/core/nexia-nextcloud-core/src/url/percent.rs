//! Percent-encoding for URI path segments and query/form values.
//!
//! Nextcloud takes user-supplied names as raw text in paths and forms, so every
//! value interpolated into a URI has to be encoded at the call site. This module
//! is the single implementation the workspace uses: the encoding rule (RFC 3986
//! unreserved set, upper-case hex) is the same for a query value, a form value
//! and a path segment, and only whether `/` survives differs.

/// Percent-encodes `value` for placement in a URI query or form value.
///
/// Only the RFC 3986 unreserved set (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`)
/// survives verbatim. In particular `&`, `=`, `+`, `%` and space are escaped, so
/// a caller-supplied value can neither add a parameter nor be reinterpreted by
/// the server as a delimiter.
pub fn percent_encode(value: &str) -> String {
    encode(value, false)
}

/// Percent-encodes `value` for placement in a URI path, keeping `/` as a
/// separator between segments.
///
/// Use this for a full path (`a/b c.txt` becomes `a/b%20c.txt`) and
/// [`percent_encode`] for one segment whose `/` is data rather than structure.
pub fn percent_encode_path(value: &str) -> String {
    encode(value, true)
}

fn encode(value: &str, keep_slash: bool) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            b'/' if keep_slash => encoded.push('/'),
            other => {
                encoded.push('%');
                encoded.push(hex_digit(other >> 4));
                encoded.push(hex_digit(other & 0x0f));
            }
        }
    }
    encoded
}

fn hex_digit(nibble: u8) -> char {
    char::from(match nibble {
        0..=9 => b'0' + nibble,
        _ => b'A' + (nibble - 10),
    })
}

#[cfg(test)]
mod tests {
    use super::{percent_encode, percent_encode_path};

    #[test]
    fn keeps_unreserved_characters() {
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("Aa0"), "Aa0");
    }

    #[test]
    fn escapes_parameter_delimiters() {
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("a+b"), "a%2Bb");
        assert_eq!(percent_encode("100%"), "100%25");
        assert_eq!(percent_encode("token=="), "token%3D%3D");
        assert_eq!(percent_encode("ä"), "%C3%A4");
    }

    #[test]
    fn path_keeps_separators() {
        assert_eq!(percent_encode_path("a/b c.txt"), "a/b%20c.txt");
        assert_eq!(
            percent_encode_path("/Photos/2024/ä.jpg"),
            "/Photos/2024/%C3%A4.jpg"
        );
    }

    #[test]
    fn path_still_escapes_segment_delimiters() {
        assert_eq!(percent_encode_path("a?b#c"), "a%3Fb%23c");
        assert_eq!(percent_encode_path("..%2F"), "..%252F");
    }
}
