use std::error::Error;
use std::fmt;
use std::path::PathBuf;

/// Failures from local storage.
///
/// Every variant names the operation and keeps the underlying cause, so a
/// locked keychain, a wrong key and a corrupt file stay distinguishable.
#[derive(Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// The configured directory or file path could not be resolved.
    Path {
        /// What was being resolved.
        operation: &'static str,
        /// The underlying cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// A database could not be opened.
    Open {
        /// File that was opened.
        path: PathBuf,
        /// The SQLite/IO cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The database rejected the key, or is not an encrypted database.
    KeyRejected {
        /// File that was opened.
        path: PathBuf,
        /// The SQLite cause (`file is not a database` and friends).
        source: Box<dyn Error + Send + Sync>,
    },
    /// A statement failed after the database was open.
    Query {
        /// Statement or schema step that failed.
        operation: &'static str,
        /// The SQLite cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// No usable key material could be produced.
    KeyMaterial {
        /// The cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The keychain could not be reached or refused the operation.
    Keychain {
        /// The `oo7`/D-Bus cause.
        source: Box<dyn Error + Send + Sync>,
    },
    /// The keychain held an entry whose secret is not a valid key.
    MalformedKey {
        /// The keychain entry label.
        entry: String,
    },
}

impl StorageError {
    pub(crate) fn path(
        operation: &'static str,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Path {
            operation,
            source: Box::new(source),
        }
    }

    pub(crate) fn open(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Self::Open {
            path: path.into(),
            source: Box::new(source),
        }
    }

    pub(crate) fn key_rejected(path: impl Into<PathBuf>, source: rusqlite::Error) -> Self {
        Self::KeyRejected {
            path: path.into(),
            source: Box::new(source),
        }
    }

    pub(crate) fn query(operation: &'static str, source: rusqlite::Error) -> Self {
        Self::Query {
            operation,
            source: Box::new(source),
        }
    }

    pub(crate) fn key_material(source: impl Error + Send + Sync + 'static) -> Self {
        Self::KeyMaterial {
            source: Box::new(source),
        }
    }

    pub(crate) fn keychain(source: impl Error + Send + Sync + 'static) -> Self {
        Self::Keychain {
            source: Box::new(source),
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path { operation, source } => {
                write!(
                    f,
                    "failed to resolve storage path while {operation}: {source}"
                )
            }
            Self::Open { path, source } => {
                write!(f, "failed to open database {}: {source}", path.display())
            }
            Self::KeyRejected { path, source } => write!(
                f,
                "database {} rejected the store key ({}); it may belong to another install",
                path.display(),
                source
            ),
            Self::Query { operation, source } => {
                write!(f, "storage query failed during {operation}: {source}")
            }
            Self::KeyMaterial { source } => write!(f, "failed to obtain key material: {source}"),
            Self::Keychain { source } => write!(f, "keychain access failed: {source}"),
            Self::MalformedKey { entry } => {
                write!(
                    f,
                    "keychain entry `{entry}` does not hold a valid store key"
                )
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Path { source, .. }
            | Self::Open { source, .. }
            | Self::KeyRejected { source, .. }
            | Self::Query { source, .. }
            | Self::KeyMaterial { source }
            | Self::Keychain { source } => Some(source.as_ref()),
            Self::MalformedKey { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::StorageError;

    #[test]
    fn open_errors_name_the_file_and_keep_the_cause() {
        let error = StorageError::open("/tmp/x.db", rusqlite::Error::InvalidQuery);
        let rendered = error.to_string();
        assert!(rendered.contains("/tmp/x.db"));
        assert!(error.source().is_some());
    }

    #[test]
    fn malformed_key_names_the_entry_without_its_secret() {
        let error = StorageError::MalformedKey {
            entry: "dev.example.app/nextcloud-rust-sdk".to_owned(),
        };
        assert!(error.to_string().contains("nextcloud-rust-sdk"));
    }
}
