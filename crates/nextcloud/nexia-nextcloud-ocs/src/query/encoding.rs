/// Percent-encodes one query key or value.
///
/// Only the RFC 3986 unreserved set survives verbatim; everything else,
/// including `&`, `=` and `+`, is escaped so a caller-supplied value cannot
/// inject an extra parameter or be reinterpreted as a space by the server.
pub(super) fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
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

fn hex_digit(nibble: u8) -> char {
    char::from(match nibble {
        0..=9 => b'0' + nibble,
        _ => b'A' + (nibble - 10),
    })
}

#[cfg(test)]
mod tests {
    use super::encode_component;

    #[test]
    fn keeps_unreserved_characters() {
        assert_eq!(
            encode_component("cloud/capabilities"),
            "cloud%2Fcapabilities"
        );
        assert_eq!(encode_component("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn escapes_parameter_delimiters() {
        assert_eq!(encode_component("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode_component("a b"), "a%20b");
        assert_eq!(encode_component("ä"), "%C3%A4");
    }
}
