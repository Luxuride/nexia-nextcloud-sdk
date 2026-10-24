/// Header carrying a WebDAV destination URI (RFC 4918 §10.3).
pub const DESTINATION_HEADER: &str = "Destination";

#[cfg(test)]
mod tests {
    use super::DESTINATION_HEADER;

    #[test]
    fn header_name_keeps_its_canonical_spelling() {
        assert_eq!(DESTINATION_HEADER, "Destination");
    }
}
