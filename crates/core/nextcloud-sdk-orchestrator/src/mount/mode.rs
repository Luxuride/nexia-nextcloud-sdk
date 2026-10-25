//! Mount mode variants defining how the mount point behaves.

use serde::{Deserialize, Serialize};

/// Mode determining how a mount point synchronizes files between local and remote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MountMode {
    /// Virtual filesystem mount: maintains placeholder metadata and hydrates file contents on demand.
    #[default]
    VirtualVfs,

    /// Bidirectional two-way synchronization: mirrors files between local target and remote Nextcloud path.
    BidirectionalSync,

    /// User-space FUSE filesystem mount directly presenting the Nextcloud hierarchy.
    Fuse,
}
