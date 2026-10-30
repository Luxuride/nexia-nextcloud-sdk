//! Mount descriptor defining the configuration of a Nextcloud mount point.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::mount::{MountId, MountMode};

/// Specification of a configured mount point connecting a Nextcloud remote hierarchy to a local directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountDescriptor {
    /// Unique identifier for this mount configuration.
    pub id: MountId,

    /// Identifier of the associated Nextcloud account.
    pub account_id: String,

    /// Remote Nextcloud directory path (e.g., `"/"` for root, or `"/Photos"`, `"/Documents"` for subdirectories).
    pub remote_path: String,

    /// Destination directory on the local filesystem where the mount is attached.
    pub local_target: PathBuf,

    /// Mount synchronization mode.
    pub mode: MountMode,

    /// Optional user-visible label for this mount.
    pub display_name: Option<String>,

    /// Whether this mount point is active and participating in automatic synchronization.
    pub enabled: bool,
}

/// Makes an identifier safe to use as a single path component.
///
/// Account identifiers come from the server's login flow and mount identifiers from the API, so
/// none of them can be trusted to be a plain name: a `/` would escape the cache root and a `.` or
/// `..` would alias another directory.
fn sanitize_component(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();

    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "default".to_string()
    } else {
        cleaned
    }
}

impl MountDescriptor {
    /// Creates a new mount descriptor connecting a remote path to a local target directory.
    #[must_use]
    pub fn new(
        account_id: impl Into<String>,
        remote_path: impl Into<String>,
        local_target: impl Into<PathBuf>,
        mode: MountMode,
    ) -> Self {
        let remote_str = remote_path.into();
        let cleaned_remote = if remote_str.is_empty() || !remote_str.starts_with('/') {
            format!("/{remote_str}")
        } else {
            remote_str
        };

        Self {
            id: MountId::new(),
            account_id: account_id.into(),
            remote_path: cleaned_remote,
            local_target: local_target.into(),
            mode,
            display_name: None,
            enabled: true,
        }
    }

    /// Assigns a custom display name to this mount descriptor.
    #[must_use]
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Sets the custom mount identifier.
    #[must_use]
    pub fn with_id(mut self, id: MountId) -> Self {
        self.id = id;
        self
    }

    /// Returns the directory holding this mount's `tree.json` and hydrated file content.
    ///
    /// Host applications pass one cache root and the SDK organises it per account, so that
    /// two accounts mounting the same remote path and two mounts of one account never share
    /// state: `<base>/accounts/<account>/mounts/<mount>`. The account and mount identifiers are
    /// sanitised, because they end up as path components and an account id may contain `/`.
    #[must_use]
    pub fn cache_dir_in(&self, base_cache_dir: &Path) -> PathBuf {
        base_cache_dir
            .join("accounts")
            .join(sanitize_component(&self.account_id))
            .join("mounts")
            .join(sanitize_component(self.id.as_str()))
    }

    /// Returns `true` if this mount attaches to the Nextcloud root directory (`"/"`).
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.remote_path == "/"
    }

    /// Resolves an absolute local path inside this mount's local target to a relative path within the mount.
    #[must_use]
    pub fn relative_path(&self, absolute_path: &Path) -> Option<PathBuf> {
        absolute_path
            .strip_prefix(&self.local_target)
            .ok()
            .map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(account: &str, id: &str) -> MountDescriptor {
        MountDescriptor::new(account, "/Photos", "/tmp/mnt", MountMode::VirtualVfs)
            .with_id(MountId::from(id))
    }

    #[test]
    fn cache_dir_is_keyed_by_account_and_mount() {
        let base = Path::new("/home/u/.cache/nexia");
        let one = mount("alice", "mount-a").cache_dir_in(base);
        let two = mount("alice", "mount-b").cache_dir_in(base);
        let other = mount("bob", "mount-a").cache_dir_in(base);

        assert_eq!(one, base.join("accounts/alice/mounts/mount-a"));
        assert_ne!(one, two, "two mounts of one account must not share state");
        assert_ne!(
            one, other,
            "one mount name in two accounts must not share state"
        );
    }

    #[test]
    fn cache_dir_stays_inside_the_base_directory() {
        let base = Path::new("/home/u/.cache/nexia");
        for hostile in ["../../etc", "a/b", "..", ".", ""] {
            let dir = mount(hostile, "mount-a").cache_dir_in(base);
            assert!(
                dir.starts_with(base),
                "account id {hostile:?} escaped the cache root: {dir:?}"
            );
            assert!(
                !dir.components().any(|c| c.as_os_str() == ".."),
                "account id {hostile:?} produced a parent component: {dir:?}"
            );
        }
    }

    #[test]
    fn sanitize_component_falls_back_for_empty_names() {
        assert_eq!(sanitize_component(""), "default");
        assert_eq!(sanitize_component("."), "default");
        assert_eq!(sanitize_component(".."), "default");
        assert_eq!(
            sanitize_component("sso-nextcloud.luxuride.eu-luxuride"),
            "sso-nextcloud.luxuride.eu-luxuride"
        );
        assert_eq!(sanitize_component("a b/c"), "a_b_c");
    }
}
