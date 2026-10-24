use std::path::PathBuf;

/// Discovers candidate file paths for the official Nextcloud desktop client configuration
/// across Linux, macOS, and Windows.
pub fn discover_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Linux & Unix standard XDG config paths
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        let base = PathBuf::from(config_home);
        paths.push(base.join("Nextcloud").join("nextcloud.cfg"));
        paths.push(base.join("ownCloud").join("owncloud.cfg"));
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        // Standard desktop client
        paths.push(
            home_path
                .join(".config")
                .join("Nextcloud")
                .join("nextcloud.cfg"),
        );
        // Sandboxed Flatpak desktop client
        paths.push(
            home_path
                .join(".var")
                .join("app")
                .join("com.nextcloud.desktopclient.nextcloud")
                .join("config")
                .join("Nextcloud")
                .join("nextcloud.cfg"),
        );
        // Legacy ownCloud client
        paths.push(
            home_path
                .join(".config")
                .join("ownCloud")
                .join("owncloud.cfg"),
        );
        // macOS standard preferences locations
        paths.push(
            home_path
                .join("Library")
                .join("Preferences")
                .join("Nextcloud")
                .join("nextcloud.cfg"),
        );
        paths.push(
            home_path
                .join("Library")
                .join("Application Support")
                .join("Nextcloud")
                .join("nextcloud.cfg"),
        );
    }

    // 2. Windows standard APPDATA paths
    if let Ok(appdata) = std::env::var("APPDATA") {
        let appdata_path = PathBuf::from(appdata);
        paths.push(appdata_path.join("Nextcloud").join("nextcloud.cfg"));
        paths.push(appdata_path.join("ownCloud").join("owncloud.cfg"));
    }

    paths
}

/// Returns the first existing desktop client configuration path found on the system, if any.
pub fn find_active_config_path() -> Option<PathBuf> {
    discover_config_paths().into_iter().find(|p| p.is_file())
}
