//! Path and identifier validation for WebDAV versions endpoints.

use sdk_core::UrlError;

/// Validates a user identifier for use in WebDAV URL paths.
///
/// Rejects empty user strings, path traversal tokens (`.` and `..`),
/// path separators (`/`), and control/URI-reserved characters.
pub(crate) fn validate_user(user: &str) -> Result<&str, UrlError> {
    let user = user.trim();
    if user.is_empty() || user == "." || user == ".." || user.contains('/') {
        return Err(UrlError::InvalidSegment(user.to_owned()));
    }
    check_characters(user)?;
    Ok(user)
}

/// Validates a revision identifier for use in WebDAV URL paths.
///
/// Rejects empty revision strings, path traversal tokens (`.` and `..`),
/// path separators (`/`), and control/URI-reserved characters.
pub(crate) fn validate_revision(revision_id: &str) -> Result<&str, UrlError> {
    let revision_id = revision_id.trim();
    if revision_id.is_empty()
        || revision_id == "."
        || revision_id == ".."
        || revision_id.contains('/')
    {
        return Err(UrlError::InvalidSegment(revision_id.to_owned()));
    }
    check_characters(revision_id)?;
    Ok(revision_id)
}

/// Checks that a segment does not contain control or URI-unsafe characters.
fn check_characters(segment: &str) -> Result<(), UrlError> {
    let rejected = segment
        .chars()
        .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'));
    if rejected {
        return Err(UrlError::InvalidSegment(segment.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_revision, validate_user};

    #[test]
    fn valid_user_and_revision_accepted() {
        assert_eq!(validate_user("alice").unwrap(), "alice");
        assert_eq!(validate_user("  bob  ").unwrap(), "bob");
        assert_eq!(validate_revision("1609459200").unwrap(), "1609459200");
    }

    #[test]
    fn empty_user_or_revision_rejected() {
        assert!(validate_user("").is_err());
        assert!(validate_user("   ").is_err());
        assert!(validate_revision("").is_err());
        assert!(validate_revision("   ").is_err());
    }

    #[test]
    fn traversal_and_separators_rejected() {
        assert!(validate_user(".").is_err());
        assert!(validate_user("..").is_err());
        assert!(validate_user("user/extra").is_err());
        assert!(validate_revision(".").is_err());
        assert!(validate_revision("..").is_err());
        assert!(validate_revision("123/456").is_err());
    }

    #[test]
    fn invalid_characters_rejected() {
        assert!(validate_user("alice?query").is_err());
        assert!(validate_user("alice#frag").is_err());
        assert!(validate_user("alice\\win").is_err());
        assert!(validate_revision("rev?1").is_err());
        assert!(validate_revision("rev#1").is_err());
    }
}
