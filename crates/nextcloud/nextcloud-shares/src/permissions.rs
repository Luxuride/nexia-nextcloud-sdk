use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// Nextcloud share permissions bitmask.
///
/// Permissions control what actions the recipient can perform on the shared
/// file or folder. Nextcloud represents these as combined bitflags on the wire.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SharePermissions(u32);

impl SharePermissions {
    /// Read permission (`1`): view and download content.
    pub const READ: Self = Self(1);
    /// Update permission (`2`): modify or overwrite existing content.
    pub const UPDATE: Self = Self(2);
    /// Create permission (`4`): add new files or subfolders.
    pub const CREATE: Self = Self(4);
    /// Delete permission (`8`): delete files or subfolders.
    pub const DELETE: Self = Self(8);
    /// Reshare permission (`16`): share the item forward to other recipients.
    pub const SHARE: Self = Self(16);
    /// All permissions combined (`31`): read, update, create, delete, and share.
    pub const ALL: Self = Self(31);

    /// An empty set of permissions (no rights granted).
    pub const fn empty() -> Self {
        Self(0)
    }

    /// All standard permissions combined.
    pub const fn all() -> Self {
        Self::ALL
    }

    /// Creates permissions from a raw bitmask.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw bitmask representation.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether this set has no permissions.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether all flags in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Whether the read permission is set.
    pub const fn is_read(self) -> bool {
        self.contains(Self::READ)
    }

    /// Whether the update permission is set.
    pub const fn is_update(self) -> bool {
        self.contains(Self::UPDATE)
    }

    /// Whether the create permission is set.
    pub const fn is_create(self) -> bool {
        self.contains(Self::CREATE)
    }

    /// Whether the delete permission is set.
    pub const fn is_delete(self) -> bool {
        self.contains(Self::DELETE)
    }

    /// Whether the reshare permission is set.
    pub const fn is_share(self) -> bool {
        self.contains(Self::SHARE)
    }

    /// Whether all standard permissions are set.
    pub const fn is_all(self) -> bool {
        self.contains(Self::ALL)
    }

    /// Adds the given permissions.
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Clears the given permissions.
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

impl BitOr for SharePermissions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for SharePermissions {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for SharePermissions {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for SharePermissions {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl BitXor for SharePermissions {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for SharePermissions {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl Not for SharePermissions {
    type Output = Self;

    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl From<u32> for SharePermissions {
    fn from(bits: u32) -> Self {
        Self::from_bits(bits)
    }
}

impl From<SharePermissions> for u32 {
    fn from(perms: SharePermissions) -> Self {
        perms.bits()
    }
}

impl fmt::Debug for SharePermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names = Vec::new();
        if self.contains(Self::READ) {
            names.push("READ");
        }
        if self.contains(Self::UPDATE) {
            names.push("UPDATE");
        }
        if self.contains(Self::CREATE) {
            names.push("CREATE");
        }
        if self.contains(Self::DELETE) {
            names.push("DELETE");
        }
        if self.contains(Self::SHARE) {
            names.push("SHARE");
        }
        if names.is_empty() {
            write!(f, "SharePermissions(NONE [0])")
        } else {
            write!(f, "SharePermissions({} [{}])", names.join(" | "), self.0)
        }
    }
}

impl fmt::Display for SharePermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for SharePermissions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.bits())
    }
}

impl<'de> Deserialize<'de> for SharePermissions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PermissionsVisitor;

        impl serde::de::Visitor<'_> for PermissionsVisitor {
            type Value = SharePermissions;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a Nextcloud permissions bitmask (integer or string)")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<SharePermissions, E> {
                let bits = u32::try_from(v)
                    .map_err(|_| E::custom(format!("permissions bitmask {v} out of range")))?;
                Ok(SharePermissions::from_bits(bits))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<SharePermissions, E> {
                let bits = u32::try_from(v)
                    .map_err(|_| E::custom(format!("permissions bitmask {v} out of range")))?;
                Ok(SharePermissions::from_bits(bits))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<SharePermissions, E> {
                let bits: u32 = v
                    .trim()
                    .parse()
                    .map_err(|_| E::custom(format!("invalid permissions string '{v}'")))?;
                Ok(SharePermissions::from_bits(bits))
            }
        }

        deserializer.deserialize_any(PermissionsVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::SharePermissions;

    #[test]
    fn constants_match_nextcloud_values() {
        assert_eq!(SharePermissions::READ.bits(), 1);
        assert_eq!(SharePermissions::UPDATE.bits(), 2);
        assert_eq!(SharePermissions::CREATE.bits(), 4);
        assert_eq!(SharePermissions::DELETE.bits(), 8);
        assert_eq!(SharePermissions::SHARE.bits(), 16);
        assert_eq!(SharePermissions::ALL.bits(), 31);
    }

    #[test]
    fn bitwise_operations_and_contains() {
        let read_update = SharePermissions::READ | SharePermissions::UPDATE;
        assert_eq!(read_update.bits(), 3);
        assert!(read_update.is_read());
        assert!(read_update.is_update());
        assert!(!read_update.is_create());
        assert!(!read_update.is_delete());
        assert!(!read_update.is_share());
        assert!(!read_update.is_all());

        let all = SharePermissions::ALL;
        assert!(all.is_all());
        assert!(all.contains(SharePermissions::READ));
        assert!(all.contains(SharePermissions::UPDATE));
        assert!(all.contains(SharePermissions::CREATE));
        assert!(all.contains(SharePermissions::DELETE));
        assert!(all.contains(SharePermissions::SHARE));

        let mut perms = SharePermissions::empty();
        perms.insert(SharePermissions::READ);
        perms.insert(SharePermissions::SHARE);
        assert_eq!(perms.bits(), 17);
        perms.remove(SharePermissions::READ);
        assert_eq!(perms.bits(), 16);
    }

    #[test]
    fn serde_json_round_trips() {
        let perms = SharePermissions::READ | SharePermissions::CREATE;
        let json = serde_json::to_string(&perms).unwrap();
        assert_eq!(json, "5");
        let parsed: SharePermissions = serde_json::from_str("5").unwrap();
        assert_eq!(parsed, perms);
    }

    #[test]
    fn serde_json_deserializes_from_string() {
        let parsed: SharePermissions = serde_json::from_str("\"31\"").unwrap();
        assert_eq!(parsed, SharePermissions::ALL);
    }

    #[test]
    fn debug_and_display() {
        let perms = SharePermissions::READ | SharePermissions::SHARE;
        let dbg = format!("{perms:?}");
        assert!(dbg.contains("READ"));
        assert!(dbg.contains("SHARE"));
        assert_eq!(perms.to_string(), "17");

        let empty = SharePermissions::empty();
        assert!(format!("{empty:?}").contains("NONE"));
    }
}
