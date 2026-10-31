//! Rules for the parts of a [`DavPath`](super::DavPath).

use nexia_nextcloud_core::UrlError;

/// Rejects user names that would change the shape of the request path.
///
/// A user name is a single path segment, so a `/` in it would move the
/// request into a different tree; `.` and `..` would traverse out of it.
pub(super) fn validate_user(user: &str) -> Result<&str, UrlError> {
    let user = user.trim();
    if user.is_empty() || user == "." || user == ".." || user.contains('/') {
        return Err(UrlError::InvalidSegment(user.to_owned()));
    }
    check_characters(user)?;
    Ok(user)
}

/// Normalises a path below the user root to the form used on the wire.
///
/// The input is relative by contract, so a leading `/` is a mistake rather
/// than something to strip: silently accepted, `"/files/alice"` would end up
/// below the user root and address a resource nobody named.
pub(super) fn validate_relative(relative: &str) -> Result<String, UrlError> {
    let relative = relative.trim();
    if relative.starts_with('/') {
        return Err(UrlError::InvalidSegment("/".to_owned()));
    }
    let relative = relative.strip_suffix('/').unwrap_or(relative);
    if relative.is_empty() {
        return Ok(String::new());
    }
    for segment in relative.split('/') {
        validate_segment(segment)?;
    }
    Ok(relative.to_owned())
}

fn validate_segment(segment: &str) -> Result<(), UrlError> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Err(UrlError::InvalidSegment(segment.to_owned()));
    }
    check_characters(segment)
}

/// Rejects characters that cannot appear in a stored name at all.
///
/// Everything else is allowed and percent-encoded on the wire by [`super::DavPath`], so a name
/// containing a space, `%`, `?`, `#` or a non-ASCII letter is addressable instead of rejected: on
/// the wire it is escaped, and the server decodes it back to the same file name. A backslash stays
/// rejected because Nextcloud and the desktop clients disagree about whether it is a separator.
fn check_characters(segment: &str) -> Result<(), UrlError> {
    let rejected = segment.chars().any(|c| c.is_control() || matches!(c, '\\'));
    if rejected {
        return Err(UrlError::InvalidSegment(segment.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_relative, validate_user};

    #[test]
    fn accepts_a_nested_relative_path() {
        assert_eq!(
            validate_relative("Photos/2024/img.jpg").unwrap(),
            "Photos/2024/img.jpg"
        );
    }

    #[test]
    fn empty_and_slash_only_paths_are_the_root() {
        assert_eq!(validate_relative("").unwrap(), "");
        assert_eq!(validate_relative("   ").unwrap(), "");
    }

    #[test]
    fn trailing_slash_is_dropped() {
        assert_eq!(validate_relative("Photos/").unwrap(), "Photos");
    }

    #[test]
    fn rejects_traversal_and_empty_segments() {
        assert!(validate_relative("Photos/../etc").is_err());
        assert!(validate_relative("Photos/./img").is_err());
        assert!(validate_relative("Photos//img").is_err());
    }

    #[test]
    fn rejects_an_absolute_path() {
        assert!(validate_relative("/files/alice").is_err());
    }

    #[test]
    fn names_that_need_escaping_are_accepted() {
        // They are encoded on the wire, so rejecting them would make real file names
        // unaddressable. `%` in particular must survive: `a%2Fb` is a name, not a separator.
        assert_eq!(
            validate_relative("My Photo #1 100%.txt").unwrap(),
            "My Photo #1 100%.txt"
        );
        assert_eq!(validate_relative("worum? geht's").unwrap(), "worum? geht's");
        assert_eq!(
            validate_relative("Fotos/Grüße.jpg").unwrap(),
            "Fotos/Grüße.jpg"
        );
    }

    #[test]
    fn rejects_backslashes_and_control_characters() {
        assert!(validate_relative("Photos\\img.jpg").is_err());
        assert!(validate_relative("Photos/img\u{7}.jpg").is_err());
    }

    #[test]
    fn user_names_are_one_segment() {
        assert_eq!(validate_user(" alice ").unwrap(), "alice");
        assert!(validate_user("").is_err());
        assert!(validate_user("..").is_err());
        assert!(validate_user("alice/eve").is_err());
    }
}
