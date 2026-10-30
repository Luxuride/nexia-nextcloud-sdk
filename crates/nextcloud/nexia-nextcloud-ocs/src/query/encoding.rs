/// Percent-encodes one query key or value.
///
/// Delegates to [`nexia_nextcloud_core::url::percent_encode`], the single
/// implementation the workspace uses, so an OCS parameter and a DAV path
/// segment cannot drift apart in what they escape.
pub(super) fn encode_component(value: &str) -> String {
    nexia_nextcloud_core::url::percent_encode(value)
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
