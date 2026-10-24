//! Text and attribute-value escaping.

/// Escapes the five characters that are never literal in XML content.
///
/// Used for both element text and attribute values, so a single function is
/// enough: `&`, `<`, `>` plus both quote kinds.
pub fn escape_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Reverses [`escape_text`], including numeric character references.
///
/// An entity the function does not recognise is left in place verbatim, so a
/// body with an unknown `&foo;` survives instead of losing characters.
pub fn unescape_text(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        decoded.push_str(&rest[..index]);
        rest = &rest[index..];
        match decode_entity(rest) {
            Some((character, consumed)) => {
                decoded.push(character);
                rest = &rest[consumed..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

/// Decodes one `&...;` reference at the start of `input`, returning the
/// character and the number of bytes consumed.
fn decode_entity(input: &str) -> Option<(char, usize)> {
    let end = input.find(';')?;
    // A reference longer than this is not a character reference, it is a
    // stray `&` followed by prose.
    if end > 12 {
        return None;
    }
    let body = &input[1..end];
    let character = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        numeric => char::from_u32(decode_number(numeric)?)?,
    };
    Some((character, end + 1))
}

fn decode_number(body: &str) -> Option<u32> {
    let (digits, radix) = match body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        Some(hex) => (hex, 16),
        None => (body.strip_prefix('#')?, 10),
    };
    u32::from_str_radix(digits, radix).ok()
}

#[cfg(test)]
mod tests {
    use super::{escape_text, unescape_text};

    #[test]
    fn escapes_every_special_character() {
        assert_eq!(
            escape_text("a & b < c > d \" e ' f"),
            "a &amp; b &lt; c &gt; d &quot; e &apos; f"
        );
    }

    #[test]
    fn escapes_a_filename_with_ampersand_and_angle_bracket() {
        assert_eq!(
            escape_text("Q1 & Q2 <final>.txt"),
            "Q1 &amp; Q2 &lt;final&gt;.txt"
        );
    }

    #[test]
    fn leaves_ordinary_text_untouched() {
        assert_eq!(escape_text("Photos/2024"), "Photos/2024");
    }

    #[test]
    fn round_trips_escaped_text() {
        let original = "Tom & Jerry's \"best\" <take>";
        assert_eq!(unescape_text(&escape_text(original)), original);
    }

    #[test]
    fn decodes_numeric_references() {
        assert_eq!(unescape_text("&#65;&#x42;"), "AB");
    }

    #[test]
    fn keeps_unknown_entities_verbatim() {
        assert_eq!(unescape_text("a &unknown; b"), "a &unknown; b");
        assert_eq!(unescape_text("a & b"), "a & b");
    }
}
