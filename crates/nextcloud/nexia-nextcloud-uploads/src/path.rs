//! URL and path helpers for Nextcloud WebDAV Chunked Upload v2.

use nexia_nextcloud_core::{ApiPath, UrlError};
use nexia_nextcloud_dav::DavPath;

/// Validates that a user identifier is suitable as a single URL path segment.
pub fn validate_user(user_id: &str) -> Result<&str, UrlError> {
    let trimmed = user_id.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains('/') {
        return Err(UrlError::InvalidSegment(user_id.to_owned()));
    }
    check_characters(trimmed)?;
    Ok(trimmed)
}

/// Validates that an upload identifier is suitable as a single URL path segment.
pub fn validate_upload_id(upload_id: &str) -> Result<&str, UrlError> {
    let trimmed = upload_id.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains('/') {
        return Err(UrlError::InvalidSegment(upload_id.to_owned()));
    }
    check_characters(trimmed)?;
    Ok(trimmed)
}

/// Validates that a chunk index is valid.
///
/// Nextcloud Chunking v2 protocol numbers chunks starting from 1 up to 10,000.
pub fn validate_chunk_index(chunk_index: u32) -> Result<u32, UrlError> {
    if chunk_index == 0 {
        return Err(UrlError::InvalidSegment(
            "chunk index must be greater than zero".to_owned(),
        ));
    }
    Ok(chunk_index)
}

/// Resolves and validates a destination path or URI for the `Destination` header.
///
/// If `destination_path` is already a full URI (e.g. starting with `http://` or `https://`)
/// or an absolute WebDAV path (starting with `/remote.php/dav/`), it is validated and returned.
///
/// Otherwise, it is treated as a path relative to the user's root and transformed into
/// `/remote.php/dav/files/{user_id}/{destination_path}` via [`DavPath`].
pub fn destination_header_value(user_id: &str, destination_path: &str) -> Result<String, UrlError> {
    let trimmed = destination_path.trim();
    if trimmed.is_empty() {
        return Err(UrlError::InvalidSegment(
            "destination path cannot be empty".to_owned(),
        ));
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        check_characters(trimmed)?;
        return Ok(trimmed.to_owned());
    }

    if trimmed.starts_with("/remote.php/dav/") {
        check_characters(trimmed)?;
        return Ok(trimmed.to_owned());
    }

    let relative = trimmed.trim_start_matches('/');
    if relative.is_empty() {
        return Err(UrlError::InvalidSegment(
            "destination path cannot be a bare slash".to_owned(),
        ));
    }

    let dav_path = DavPath::new(user_id, relative)?;
    Ok(dav_path.as_str().to_owned())
}

/// Rejects control characters and URL delimiters that cannot appear literally in paths.
fn check_characters(segment: &str) -> Result<(), UrlError> {
    let rejected = segment
        .chars()
        .any(|c| c.is_control() || matches!(c, '?' | '#' | '\\'));
    if rejected {
        return Err(UrlError::InvalidSegment(segment.to_owned()));
    }
    Ok(())
}

/// Builds the [`ApiPath`] addressing the upload collection directory.
///
/// Resulting path: `/remote.php/dav/uploads/{user_id}/{upload_id}`
pub fn upload_collection_path(user_id: &str, upload_id: &str) -> Result<ApiPath, UrlError> {
    let user = validate_user(user_id)?;
    let upload = validate_upload_id(upload_id)?;
    ApiPath::dav(format!("uploads/{user}/{upload}"))
}

/// Builds the [`ApiPath`] addressing a single chunk in the upload collection.
///
/// Formats the chunk index as a 5-digit zero-padded number (e.g., `00001`).
/// Resulting path: `/remote.php/dav/uploads/{user_id}/{upload_id}/{chunk_number}`
pub fn chunk_path(user_id: &str, upload_id: &str, chunk_index: u32) -> Result<ApiPath, UrlError> {
    let user = validate_user(user_id)?;
    let upload = validate_upload_id(upload_id)?;
    let valid_index = validate_chunk_index(chunk_index)?;
    let chunk_segment = format!("{valid_index:05}");
    ApiPath::dav(format!("uploads/{user}/{upload}/{chunk_segment}"))
}

/// Builds the [`ApiPath`] addressing the assembly target file.
///
/// Resulting path: `/remote.php/dav/uploads/{user_id}/{upload_id}/.file`
pub fn assembly_path(user_id: &str, upload_id: &str) -> Result<ApiPath, UrlError> {
    let user = validate_user(user_id)?;
    let upload = validate_upload_id(upload_id)?;
    ApiPath::dav(format!("uploads/{user}/{upload}/.file"))
}

/// Builds the [`ApiPath`] addressing the upload directory to abort and delete it.
///
/// Resulting path: `/remote.php/dav/uploads/{user_id}/{upload_id}/`
pub fn abort_path(user_id: &str, upload_id: &str) -> Result<ApiPath, UrlError> {
    let user = validate_user(user_id)?;
    let upload = validate_upload_id(upload_id)?;
    ApiPath::dav(format!("uploads/{user}/{upload}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_paths_construction() {
        let collection = upload_collection_path("alice", "up-123").unwrap();
        assert_eq!(collection.as_str(), "/remote.php/dav/uploads/alice/up-123");

        let chunk1 = chunk_path("alice", "up-123", 1).unwrap();
        assert_eq!(
            chunk1.as_str(),
            "/remote.php/dav/uploads/alice/up-123/00001"
        );

        let chunk42 = chunk_path("alice", "up-123", 42).unwrap();
        assert_eq!(
            chunk42.as_str(),
            "/remote.php/dav/uploads/alice/up-123/00042"
        );

        let assembly = assembly_path("alice", "up-123").unwrap();
        assert_eq!(
            assembly.as_str(),
            "/remote.php/dav/uploads/alice/up-123/.file"
        );

        let abort = abort_path("alice", "up-123").unwrap();
        assert_eq!(abort.as_str(), "/remote.php/dav/uploads/alice/up-123/");
    }

    #[test]
    fn destination_header_resolution() {
        let rel = destination_header_value("alice", "Photos/trip.jpg").unwrap();
        assert_eq!(rel, "/remote.php/dav/files/alice/Photos/trip.jpg");

        let leading_slash_rel = destination_header_value("alice", "/Photos/trip.jpg").unwrap();
        assert_eq!(
            leading_slash_rel,
            "/remote.php/dav/files/alice/Photos/trip.jpg"
        );

        let full_dav =
            destination_header_value("alice", "/remote.php/dav/files/alice/Photos/trip.jpg")
                .unwrap();
        assert_eq!(full_dav, "/remote.php/dav/files/alice/Photos/trip.jpg");

        let full_url = destination_header_value(
            "alice",
            "https://cloud.example.com/remote.php/dav/files/alice/Photos/trip.jpg",
        )
        .unwrap();
        assert_eq!(
            full_url,
            "https://cloud.example.com/remote.php/dav/files/alice/Photos/trip.jpg"
        );
    }

    #[test]
    fn rejects_invalid_segments() {
        assert!(validate_user("").is_err());
        assert!(validate_user("..").is_err());
        assert!(validate_user("alice/bob").is_err());
        assert!(validate_user("alice?x").is_err());

        assert!(validate_upload_id("").is_err());
        assert!(validate_upload_id("..").is_err());
        assert!(validate_upload_id("id/nested").is_err());
        assert!(validate_upload_id("id#bad").is_err());

        assert!(validate_chunk_index(0).is_err());
        assert!(validate_chunk_index(1).is_ok());

        assert!(destination_header_value("alice", "").is_err());
        assert!(destination_header_value("alice", "/").is_err());
        assert!(destination_header_value("alice", "../traversal").is_err());
        assert!(destination_header_value("alice", "file?bad").is_err());
    }
}
