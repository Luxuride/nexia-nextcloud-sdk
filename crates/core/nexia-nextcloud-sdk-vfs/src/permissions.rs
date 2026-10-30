//! Nextcloud WebDAV resource permission capabilities parser and formatter.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Capabilities parsed from Nextcloud WebDAV `oc:permissions` flags.
///
/// Nextcloud represents resource permissions on WebDAV endpoints using
/// a set of letter codes:
/// - `R`: Read / view / download content.
/// - `W`: Write / update existing file contents.
/// - `C`: Create new files or subdirectories.
/// - `D`: Delete files or subdirectories.
/// - `N`: Rename files or subdirectories.
/// - `V`: Move files or subdirectories.
/// - `S`: Shared resource (received from another user or group).
/// - `M`: Mounted resource (external storage mount point).
/// - `G`: Shareable / group share.
/// - `K`: Reshareable (can share forward to others).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NextcloudPermissions {
    /// Can read and download content (`'R'`).
    pub can_read: bool,
    /// Can update and overwrite existing content (`'W'`).
    pub can_write: bool,
    /// Can create new files or subfolders (`'C'`).
    pub can_create: bool,
    /// Can delete the item (`'D'`).
    pub can_delete: bool,
    /// Can rename the item (`'N'`).
    pub can_rename: bool,
    /// Can move the item (`'V'`).
    pub can_move: bool,
    /// Whether the item is a shared resource (`'S'`).
    pub is_shared: bool,
    /// Whether the item is hosted on an external storage mount point (`'M'`).
    pub is_mounted: bool,
    /// Can share or reshare the item with other users (`'G'`, `'K'`, or `'S'`).
    pub can_share: bool,
}

impl NextcloudPermissions {
    /// Full access permissions granting read, write, create, delete, rename, move, and share.
    pub const FULL: Self = Self {
        can_read: true,
        can_write: true,
        can_create: true,
        can_delete: true,
        can_rename: true,
        can_move: true,
        is_shared: false,
        is_mounted: false,
        can_share: true,
    };

    /// Read-only permissions granting read and download access only.
    pub const READ_ONLY: Self = Self {
        can_read: true,
        can_write: false,
        can_create: false,
        can_delete: false,
        can_rename: false,
        can_move: false,
        is_shared: false,
        is_mounted: false,
        can_share: false,
    };

    /// Empty permissions with no rights granted.
    pub const fn empty() -> Self {
        Self {
            can_read: false,
            can_write: false,
            can_create: false,
            can_delete: false,
            can_rename: false,
            can_move: false,
            is_shared: false,
            is_mounted: false,
            can_share: false,
        }
    }

    /// Parses Nextcloud `oc:permissions` string flags (e.g. `"RDNVW"`, `"RGDNVCK"`).
    pub fn parse(flags: &str) -> Self {
        let mut perms = Self::empty();
        for ch in flags.chars() {
            match ch {
                'R' | 'r' => perms.can_read = true,
                'W' | 'w' => perms.can_write = true,
                'C' | 'c' => perms.can_create = true,
                'D' | 'd' => perms.can_delete = true,
                'N' | 'n' => perms.can_rename = true,
                'V' | 'v' => perms.can_move = true,
                'S' | 's' => {
                    perms.is_shared = true;
                    perms.can_share = true;
                }
                'M' | 'm' => perms.is_mounted = true,
                'G' | 'g' | 'K' | 'k' => perms.can_share = true,
                _ => {}
            }
        }
        perms
    }

    /// Reconstructs the canonical Nextcloud WebDAV permission string.
    pub fn raw_string(&self) -> String {
        let mut s = String::with_capacity(10);
        if self.can_read {
            s.push('R');
        }
        if self.can_share {
            s.push('G');
        }
        if self.can_delete {
            s.push('D');
        }
        if self.can_rename {
            s.push('N');
        }
        if self.can_move {
            s.push('V');
        }
        if self.can_create {
            s.push('C');
        }
        if self.can_write {
            s.push('W');
        }
        if self.can_share {
            s.push('K');
        }
        if self.is_shared {
            s.push('S');
        }
        if self.is_mounted {
            s.push('M');
        }
        s
    }

    /// Whether this set grants read and download access.
    pub const fn can_read(&self) -> bool {
        self.can_read
    }

    /// Whether this set grants updating or overwriting existing file content.
    pub const fn can_write(&self) -> bool {
        self.can_write
    }

    /// Whether this set grants creating new files or subfolders.
    pub const fn can_create(&self) -> bool {
        self.can_create
    }

    /// Whether this set grants deleting the item.
    pub const fn can_delete(&self) -> bool {
        self.can_delete
    }

    /// Whether this set grants renaming the item.
    pub const fn can_rename(&self) -> bool {
        self.can_rename
    }

    /// Whether this set grants moving the item to another folder.
    pub const fn can_move(&self) -> bool {
        self.can_move
    }

    /// Whether this set grants sharing with other users or creating public links.
    pub const fn can_share(&self) -> bool {
        self.can_share
    }

    /// Whether the item is shared by another user.
    pub const fn is_shared(&self) -> bool {
        self.is_shared
    }

    /// Whether the item resides on an external storage mount.
    pub const fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    /// Whether the user has strictly read-only access (can read, but cannot modify, create, or delete).
    pub const fn is_read_only(&self) -> bool {
        self.can_read && !self.can_write && !self.can_create && !self.can_delete
    }

    /// Whether the user has full standard read-write access.
    pub const fn is_full_access(&self) -> bool {
        self.can_read
            && (self.can_write || self.can_create)
            && self.can_delete
            && self.can_rename
            && self.can_move
    }

    /// A concise summary label suitable for UI table rows or properties dialogs.
    ///
    /// Examples: `"Read & Write (Full)"`, `"Read-Only"`, `"Read & Write"`, `"No Access"`.
    pub fn summary_label(&self) -> &'static str {
        if !self.can_read && !self.can_write && !self.can_create && !self.can_delete {
            "No Access"
        } else if self.is_full_access() {
            "Read & Write (Full)"
        } else if self.is_read_only() {
            "Read-Only"
        } else if self.can_write || self.can_create {
            "Read & Write"
        } else if self.can_read {
            "Read-Only"
        } else {
            "Custom Access"
        }
    }

    /// A human-readable sentence describing the exact capabilities granted.
    pub fn human_readable(&self) -> String {
        if self.is_full_access() {
            if self.can_share {
                return "Full access (can read, write, delete, and share)".to_string();
            }
            return "Full access (can read, write, and delete)".to_string();
        }

        if self.is_read_only() {
            if self.can_share {
                return "Read-only access (can download and share)".to_string();
            }
            return "Read-only access (can view and download)".to_string();
        }

        let mut actions = Vec::new();
        if self.can_read {
            actions.push("read");
        }
        if self.can_write {
            actions.push("write");
        }
        if self.can_create {
            actions.push("create");
        }
        if self.can_delete {
            actions.push("delete");
        }
        if self.can_rename {
            actions.push("rename");
        }
        if self.can_move {
            actions.push("move");
        }
        if self.can_share {
            actions.push("share");
        }

        if actions.is_empty() {
            "No access permissions granted".to_string()
        } else {
            format!("Can {}", actions.join(", "))
        }
    }

    /// Returns a list of detailed capability descriptions for UI bullet points or tooltips.
    pub fn capability_descriptions(&self) -> Vec<&'static str> {
        let mut caps = Vec::new();
        if self.can_read {
            caps.push("Read and download content");
        }
        if self.can_write {
            caps.push("Modify and save changes");
        }
        if self.can_create {
            caps.push("Create new files and folders");
        }
        if self.can_delete {
            caps.push("Delete items");
        }
        if self.can_rename {
            caps.push("Rename items");
        }
        if self.can_move {
            caps.push("Move to another folder");
        }
        if self.can_share {
            caps.push("Share with others");
        }
        if self.is_shared {
            caps.push("Shared by another user");
        }
        if self.is_mounted {
            caps.push("External storage mount point");
        }
        caps
    }
}

impl From<&str> for NextcloudPermissions {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl FromStr for NextcloudPermissions {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

impl fmt::Display for NextcloudPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary_label())
    }
}

impl fmt::Debug for NextcloudPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NextcloudPermissions")
            .field("summary", &self.summary_label())
            .field("can_read", &self.can_read)
            .field("can_write", &self.can_write)
            .field("can_create", &self.can_create)
            .field("can_delete", &self.can_delete)
            .field("can_rename", &self.can_rename)
            .field("can_move", &self.can_move)
            .field("can_share", &self.can_share)
            .field("is_shared", &self.is_shared)
            .field("is_mounted", &self.is_mounted)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_permissions() {
        let perms = NextcloudPermissions::parse("RDNVW");
        assert!(perms.can_read());
        assert!(perms.can_write());
        assert!(!perms.can_create());
        assert!(perms.can_delete());
        assert!(perms.can_rename());
        assert!(perms.can_move());
        assert!(!perms.can_share());
        assert!(perms.is_full_access());
        assert_eq!(perms.summary_label(), "Read & Write (Full)");
    }

    #[test]
    fn parse_shared_folder_permissions() {
        let perms = NextcloudPermissions::parse("RGDNVCK");
        assert!(perms.can_read());
        assert!(perms.can_create());
        assert!(perms.can_delete());
        assert!(perms.can_rename());
        assert!(perms.can_move());
        assert!(perms.can_share());
        assert!(perms.is_full_access());
        assert_eq!(
            perms.human_readable(),
            "Full access (can read, write, delete, and share)"
        );
    }

    #[test]
    fn parse_read_only_permissions() {
        let perms = NextcloudPermissions::parse("R");
        assert!(perms.can_read());
        assert!(!perms.can_write());
        assert!(!perms.can_create());
        assert!(!perms.can_delete());
        assert!(perms.is_read_only());
        assert_eq!(perms.summary_label(), "Read-Only");
        assert_eq!(
            perms.human_readable(),
            "Read-only access (can view and download)"
        );
    }

    #[test]
    fn empty_permissions() {
        let perms = NextcloudPermissions::empty();
        assert!(!perms.can_read());
        assert_eq!(perms.summary_label(), "No Access");
        assert_eq!(perms.human_readable(), "No access permissions granted");
    }

    #[test]
    fn capability_descriptions_list() {
        let perms = NextcloudPermissions::parse("RSM");
        let caps = perms.capability_descriptions();
        assert!(caps.contains(&"Read and download content"));
        assert!(caps.contains(&"Share with others"));
        assert!(caps.contains(&"Shared by another user"));
        assert!(caps.contains(&"External storage mount point"));
        assert!(!caps.contains(&"Modify and save changes"));
    }
}
