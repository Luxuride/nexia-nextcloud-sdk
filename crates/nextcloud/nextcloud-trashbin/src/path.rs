//! URL and path helpers for Nextcloud WebDAV trashbin endpoints.

use nextcloud_sdk_core::{ApiPath, UrlError};

/// Validates that a user name is suitable as a single URL path segment.
pub(crate) fn validate_user_id(user_id: &str) -> Result<&str, UrlError> {
    let trimmed = user_id.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains('/') {
        return Err(UrlError::InvalidSegment(user_id.to_owned()));
    }
    check_characters(trimmed)?;
    Ok(trimmed)
}

/// Validates that a trash item identifier is suitable as a single URL path segment.
pub(crate) fn validate_item_id(item_id: &str) -> Result<&str, UrlError> {
    let trimmed = item_id.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains('/') {
        return Err(UrlError::InvalidSegment(item_id.to_owned()));
    }
    check_characters(trimmed)?;
    Ok(trimmed)
}

/// Validates and normalises a destination restore path.
pub(crate) fn validate_restore_path(restore_path: &str) -> Result<String, UrlError> {
    let trimmed = restore_path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(UrlError::InvalidSegment(segment.to_owned()));
        }
        check_characters(segment)?;
    }
    Ok(trimmed.to_owned())
}

/// Rejects control characters and URL delimiters that cannot appear literally.
fn check_characters(segment: &str) -> Result<(), UrlError> {
    let rejected = segment
        .chars()
        .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'));
    if rejected {
        return Err(UrlError::InvalidSegment(segment.to_owned()));
    }
    Ok(())
}

/// Builds the [`ApiPath`] addressing the user's trashbin root collection.
///
/// Resulting path: `/remote.php/dav/trashbin/{user_id}/trash/`
pub(crate) fn trash_root_path(user_id: &str) -> Result<ApiPath, UrlError> {
    let user = validate_user_id(user_id)?;
    ApiPath::dav(format!("trashbin/{user}/trash/"))
}

/// Builds the [`ApiPath`] addressing a single deleted item in the trashbin.
///
/// Resulting path: `/remote.php/dav/trashbin/{user_id}/trash/{item_id}`
pub(crate) fn trash_item_path(user_id: &str, item_id: &str) -> Result<ApiPath, UrlError> {
    let user = validate_user_id(user_id)?;
    let item = validate_item_id(item_id)?;
    ApiPath::dav(format!("trashbin/{user}/trash/{item}"))
}

/// Builds the [`ApiPath`] addressing the destination restore location.
///
/// When `restore_path` is empty, returns `/remote.php/dav/trashbin/{user_id}/restore`.
/// Otherwise returns `/remote.php/dav/trashbin/{user_id}/restore/{restore_path}`.
pub(crate) fn restore_destination_path(
    user_id: &str,
    restore_path: &str,
) -> Result<ApiPath, UrlError> {
    let user = validate_user_id(user_id)?;
    let path = validate_restore_path(restore_path)?;
    if path.is_empty() {
        ApiPath::dav(format!("trashbin/{user}/restore"))
    } else {
        ApiPath::dav(format!("trashbin/{user}/restore/{path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_user_and_item_paths() {
        let root = trash_root_path("alice").expect("valid user");
        assert_eq!(root.as_str(), "/remote.php/dav/trashbin/alice/trash/");

        let item = trash_item_path("alice", "file.txt.d12345").expect("valid item");
        assert_eq!(
            item.as_str(),
            "/remote.php/dav/trashbin/alice/trash/file.txt.d12345"
        );
    }

    #[test]
    fn restore_destination_paths() {
        let default_dest = restore_destination_path("alice", "").expect("empty restore path");
        assert_eq!(
            default_dest.as_str(),
            "/remote.php/dav/trashbin/alice/restore"
        );

        let slash_dest = restore_destination_path("alice", " / ").expect("whitespace/slashes");
        assert_eq!(
            slash_dest.as_str(),
            "/remote.php/dav/trashbin/alice/restore"
        );

        let custom_dest =
            restore_destination_path("alice", "Documents/notes.txt").expect("custom restore path");
        assert_eq!(
            custom_dest.as_str(),
            "/remote.php/dav/trashbin/alice/restore/Documents/notes.txt"
        );

        let leading_slash =
            restore_destination_path("alice", "/Photos/img.png").expect("leading slash");
        assert_eq!(
            leading_slash.as_str(),
            "/remote.php/dav/trashbin/alice/restore/Photos/img.png"
        );
    }

    #[test]
    fn rejects_invalid_segments() {
        assert!(trash_root_path("").is_err());
        assert!(trash_root_path("..").is_err());
        assert!(trash_root_path("alice/bob").is_err());
        assert!(trash_root_path("alice?query").is_err());

        assert!(trash_item_path("alice", "").is_err());
        assert!(trash_item_path("alice", "..").is_err());
        assert!(trash_item_path("alice", "item/nested").is_err());

        assert!(restore_destination_path("alice", "../escape").is_err());
        assert!(restore_destination_path("alice", "doc/./file").is_err());
        assert!(restore_destination_path("alice", "doc?query").is_err());
    }
}
