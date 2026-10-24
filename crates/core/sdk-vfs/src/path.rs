//! Path normalization and navigation helpers for canonical virtual paths.

use crate::error::VfsError;

/// Normalizes a path to a canonical absolute virtual mount path.
///
/// Leading slashes are preserved, redundant consecutive slashes and current-directory
/// segments (`.`) are collapsed, and trailing slashes are trimmed (except for root `"/"`).
///
/// # Errors
///
/// Returns [`VfsError::InvalidPath`] if the path contains null bytes or parent traversal
/// (`..`) components that would escape canonical boundaries.
pub fn normalize_path(path: &str) -> Result<String, VfsError> {
    if path.contains('\0') {
        return Err(VfsError::InvalidPath(
            "path contains forbidden null byte".to_string(),
        ));
    }

    if path.is_empty() || path == "/" {
        return Ok("/".to_string());
    }

    let mut segments = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(VfsError::InvalidPath(format!(
                "directory traversal with '..' is not allowed: `{path}`"
            )));
        }
        segments.push(seg);
    }

    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

/// Returns the parent path of a canonical virtual path, or `None` if the path is root.
pub fn parent_path(path: &str) -> Option<String> {
    let normalized = normalize_path(path).ok()?;
    if normalized == "/" {
        return None;
    }

    let mut parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 1 {
        Some("/".to_string())
    } else {
        parts.pop();
        Some(format!("/{}", parts.join("/")))
    }
}

/// Returns the final name component of a canonical virtual path.
pub fn file_name(path: &str) -> String {
    let normalized = normalize_path(path).unwrap_or_else(|_| path.to_string());
    if normalized == "/" {
        return "/".to_string();
    }
    normalized
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map_or_else(|| "/".to_string(), ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths_cleanly() {
        assert_eq!(normalize_path("/").unwrap(), "/");
        assert_eq!(normalize_path("").unwrap(), "/");
        assert_eq!(normalize_path("Photos").unwrap(), "/Photos");
        assert_eq!(normalize_path("/Photos/").unwrap(), "/Photos");
        assert_eq!(
            normalize_path("/Photos//vacation.jpg").unwrap(),
            "/Photos/vacation.jpg"
        );
        assert_eq!(
            normalize_path("/./Photos/./vacation.jpg").unwrap(),
            "/Photos/vacation.jpg"
        );
    }

    #[test]
    fn rejects_invalid_paths() {
        assert!(matches!(
            normalize_path("../escaped"),
            Err(VfsError::InvalidPath(_))
        ));
        assert!(matches!(
            normalize_path("/Photos/../etc"),
            Err(VfsError::InvalidPath(_))
        ));
        assert!(matches!(
            normalize_path("/Photos/\0hidden"),
            Err(VfsError::InvalidPath(_))
        ));
    }

    #[test]
    fn extracts_parent_paths() {
        assert_eq!(parent_path("/"), None);
        assert_eq!(parent_path("/Photos"), Some("/".to_string()));
        assert_eq!(
            parent_path("/Photos/vacation.jpg"),
            Some("/Photos".to_string())
        );
        assert_eq!(
            parent_path("/Photos/2026/trip/beach.png"),
            Some("/Photos/2026/trip".to_string())
        );
    }

    #[test]
    fn extracts_file_names() {
        assert_eq!(file_name("/"), "/");
        assert_eq!(file_name("/Photos"), "Photos");
        assert_eq!(file_name("/Photos/vacation.jpg"), "vacation.jpg");
    }
}
