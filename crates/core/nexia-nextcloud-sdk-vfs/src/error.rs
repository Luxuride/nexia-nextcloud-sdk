//! Error types for virtual file system operations.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Errors produced when navigating, hydrating, or modifying nodes in a virtual tree.
#[derive(Debug)]
pub enum VfsError {
    /// The requested path was not found in the virtual tree.
    NotFound(String),
    /// The path refers to a directory where a file was expected.
    NotAFile(String),
    /// The path refers to a file where a directory was expected.
    NotADirectory(String),
    /// The provided path is invalid or malformed.
    InvalidPath(String),
    /// The target path already exists in the tree.
    TargetAlreadyExists(String),
    /// Attempted to move a directory into itself or one of its descendants.
    CannotMoveIntoSelf(String),
    /// The root directory cannot be moved or renamed.
    CannotModifyRoot,
    /// A filesystem operation on the local cache failed.
    ///
    /// This is distinct from [`VfsError::NotFound`]: the node exists and is hydrated, but its
    /// cached payload cannot be read back, so a caller must not treat the node as a placeholder.
    Io {
        /// Human-readable description of the failed operation.
        operation: &'static str,
        /// Path of the file that failed.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
}

impl fmt::Display for VfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "path not found in virtual tree: `{path}`"),
            Self::NotAFile(path) => write!(f, "path is a directory, not a file: `{path}`"),
            Self::NotADirectory(path) => write!(f, "path is a file, not a directory: `{path}`"),
            Self::InvalidPath(path) => write!(f, "invalid virtual file system path: `{path}`"),
            Self::TargetAlreadyExists(path) => {
                write!(f, "target path already exists in virtual tree: `{path}`")
            }
            Self::CannotMoveIntoSelf(path) => {
                write!(
                    f,
                    "cannot move directory into itself or descendant: `{path}`"
                )
            }
            Self::CannotModifyRoot => write!(f, "cannot rename or move root directory"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "failed {operation} for `{}`: {source}", path.display()),
        }
    }
}

impl Error for VfsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VfsError;
    use std::error::Error;

    #[test]
    fn display_messages_mention_paths() {
        let err = VfsError::NotFound("/photos/sunset.jpg".into());
        assert!(err.to_string().contains("/photos/sunset.jpg"));

        let err = VfsError::NotAFile("/photos".into());
        assert!(err.to_string().contains("/photos"));

        let err = VfsError::NotADirectory("/photos/sunset.jpg".into());
        assert!(err.to_string().contains("/photos/sunset.jpg"));

        let err = VfsError::InvalidPath("../escaped".into());
        assert!(err.to_string().contains("../escaped"));

        let err = VfsError::TargetAlreadyExists("/photos".into());
        assert!(err.to_string().contains("/photos"));

        let err = VfsError::CannotMoveIntoSelf("/photos".into());
        assert!(err.to_string().contains("/photos"));

        let err = VfsError::CannotModifyRoot;
        assert_eq!(err.to_string(), "cannot rename or move root directory");
    }

    #[test]
    fn io_errors_name_the_operation_and_keep_the_cause() {
        let err = VfsError::Io {
            operation: "reading the cached payload",
            path: std::path::PathBuf::from("/cache/a.jpg"),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        assert!(err.to_string().contains("reading the cached payload"));
        assert!(err.to_string().contains("/cache/a.jpg"));
        assert!(err.source().is_some(), "the cause must stay reachable");
    }
}
