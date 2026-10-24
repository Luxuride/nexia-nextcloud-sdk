use std::path::PathBuf;

/// Discovers candidate file paths for the KDE Plasma / Accounts-SSO `accounts.db` SQLite database.
pub fn discover_accounts_db_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        paths.push(
            PathBuf::from(data_home)
                .join("accounts")
                .join("accounts.db"),
        );
    }

    if let Ok(home) = std::env::var("HOME") {
        let base = PathBuf::from(home);
        paths.push(
            base.join(".local")
                .join("share")
                .join("accounts")
                .join("accounts.db"),
        );
    }

    paths
}

/// Finds the active `accounts.db` file if one exists on the current system.
pub fn find_active_accounts_db() -> Option<PathBuf> {
    discover_accounts_db_paths()
        .into_iter()
        .find(|p| p.is_file())
}
