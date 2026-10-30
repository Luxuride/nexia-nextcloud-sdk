//! FUSE filesystem integration bridging [`FuseDriver`] to OS mounts via `fuser`.

use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use fuser::{
    AccessFlags, BackgroundSession, Config, Errno, FileHandle, Filesystem, FopenFlags, Generation,
    INodeNo, LockOwner, MountOption, OpenFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyXattr, Request,
};
use tracing::{debug, error, trace};

use crate::attr::FileType;
use crate::driver::FuseDriver;

const TTL: Duration = Duration::from_secs(60);

impl From<&crate::attr::FileAttr> for fuser::FileAttr {
    fn from(attr: &crate::attr::FileAttr) -> Self {
        fuser::FileAttr {
            ino: INodeNo(attr.ino),
            size: attr.size,
            blocks: attr.blocks,
            atime: attr.atime,
            mtime: attr.mtime,
            ctime: attr.ctime,
            crtime: attr.crtime,
            kind: match attr.kind {
                FileType::Directory => fuser::FileType::Directory,
                FileType::RegularFile => fuser::FileType::RegularFile,
            },
            perm: attr.perm,
            nlink: attr.nlink,
            uid: attr.uid,
            gid: attr.gid,
            rdev: attr.rdev,
            flags: attr.flags,
            blksize: crate::attr::BLOCK_SIZE as u32,
        }
    }
}

fn to_errno(code: i32) -> Errno {
    match code {
        2 => Errno::ENOENT,
        22 => Errno::EINVAL,
        20 => Errno::ENOTDIR,
        11 => Errno::EAGAIN,
        29 => Errno::ESPIPE,
        34 => Errno::ERANGE,
        _ => Errno::EIO,
    }
}

/// Binary names of background thumbnailer / indexer daemons.
///
/// Those daemons walk whole directories and only want a small preview, so hydrating every
/// multi-megabyte original they touch would destroy the mount. They are recognised **only** by
/// their own executable name: no ancestor walk and no file-manager matching, because both used to
/// misclassify ordinary applications (an image viewer started from the desktop shell has a
/// `gvfsd`/`nautilus` ancestor) and then deny every read with `ENODATA`.
#[cfg(target_os = "linux")]
const THUMBNAILER_COMMS: &[&str] = &["tumblerd", "thumbnailerd", "tracker-miner", "localsearch"];

/// Returns `true` when `pid` itself is a background thumbnailer or indexer daemon.
fn is_background_thumbnailer(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
            return false;
        };
        let comm = comm.trim();
        if comm.is_empty() {
            return false;
        }
        THUMBNAILER_COMMS.contains(&comm)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

impl Filesystem for FuseDriver {
    fn access(&self, _req: &Request, _ino: INodeNo, _mask: AccessFlags, reply: ReplyEmpty) {
        reply.ok();
    }

    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _lock_owner: LockOwner,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        reply.statfs(
            1_000_000,
            500_000,
            500_000,
            1_000_000,
            500_000,
            crate::attr::BLOCK_SIZE as u32,
            255,
            0,
        );
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(Errno::EINVAL);
                return;
            }
        };

        trace!(parent = parent.0, name = name_str, "fuser::lookup");
        match self.lookup(parent.0, name_str) {
            Ok(attr) => reply.entry(&TTL, &(&attr).into(), Generation(0)),
            Err(code) => reply.error(to_errno(code)),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        trace!(ino = ino.0, "fuser::getattr");
        match self.getattr(ino.0) {
            Ok(attr) => reply.attr(&TTL, &(&attr).into()),
            Err(code) => reply.error(to_errno(code)),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        trace!(ino = ino.0, offset, "fuser::readdir");
        match self.readdir(ino.0, offset as i64) {
            Ok(entries) => {
                for (i, entry) in entries.into_iter().enumerate() {
                    let kind = match entry.kind {
                        FileType::Directory => fuser::FileType::Directory,
                        FileType::RegularFile => fuser::FileType::RegularFile,
                    };
                    let next_offset = offset + 1 + i as u64;
                    if reply.add(INodeNo(entry.ino), next_offset, kind, &entry.name) {
                        break;
                    }
                }
                reply.ok();
            }
            Err(code) => reply.error(to_errno(code)),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
        trace!(ino = ino.0, "fuser::open");
        // `FOPEN_KEEP_CACHE` keeps already-hydrated bytes resident across opens. The cache is safe
        // to keep because a failed fetch now surfaces as `EIO` (see `FuseDriver::read_ranged`)
        // instead of a short read, so the kernel never stores a bogus end-of-file page.
        // `FOPEN_DIRECT_IO` is deliberately *not* set: it would disable `mmap`, which several
        // image viewers rely on.
        reply.opened(FileHandle(ino.0), FopenFlags::FOPEN_KEEP_CACHE);
    }

    fn read(
        &self,
        req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        trace!(ino = ino.0, offset, size, "fuser::read");

        // Background thumbnailers/indexers get an `EAGAIN` style "try later" answer instead of
        // triggering a full remote hydration for a preview they will throw away. Ordinary readers
        // (file managers, image viewers) are never denied: hydration is their only way to work.
        if is_background_thumbnailer(req.pid()) && self.is_placeholder(ino.0) {
            // `ECHILD` rather than `EAGAIN`. On a page-cache-backed read the kernel reserves
            // `EAGAIN` for its own internal retry and converts it into `EIO` before it reaches
            // userspace, so an `EAGAIN` here is reported to the thumbnailer as an I/O error and
            // the whole file is permanently marked unreadable (`~/.cache/thumbnails/fail`)
            // instead of being retried once it is hydrated. `ECHILD` is passed through verbatim,
            // which is what a thumbnailer can actually act on.
            debug!(
                pid = req.pid(),
                ino = ino.0,
                offset,
                "Deferring thumbnailer read on unhydrated placeholder"
            );
            reply.error(Errno::ECHILD);
            return;
        }
        let result = if let Some(rt) = self.runtime_handle() {
            Self::execute_blocking(&rt, self.read(ino.0, offset as i64, size))
        } else {
            match tokio::runtime::Handle::try_current() {
                Ok(rt) => Self::execute_blocking(&rt, self.read(ino.0, offset as i64, size)),
                Err(_) => Err(5), // EIO
            }
        };

        match result {
            Ok(ref bytes) => reply.data(bytes),
            Err(code) => {
                error!(
                    ino = ino.0,
                    offset,
                    size,
                    error_code = code,
                    "fuser::read error"
                );
                reply.error(to_errno(code));
            }
        }
    }

    fn getxattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, size: u32, reply: ReplyXattr) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(Errno::EINVAL);
                return;
            }
        };

        trace!(ino = ino.0, name = name_str, size, "fuser::getxattr");
        match self.getxattr(ino.0, name_str) {
            Ok(bytes) => {
                if size == 0 {
                    reply.size(bytes.len() as u32);
                } else if (size as usize) < bytes.len() {
                    reply.error(Errno::ERANGE);
                } else {
                    reply.data(&bytes);
                }
            }
            Err(code) => reply.error(to_errno(code)),
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        trace!(ino = ino.0, size, "fuser::listxattr");
        match self.listxattr(ino.0) {
            Ok(attrs) => {
                let mut buffer = Vec::new();
                for attr in attrs {
                    buffer.extend_from_slice(attr.as_bytes());
                    buffer.push(0);
                }
                if size == 0 {
                    reply.size(buffer.len() as u32);
                } else if (size as usize) < buffer.len() {
                    reply.error(Errno::ERANGE);
                } else {
                    reply.data(&buffer);
                }
            }
            Err(code) => reply.error(to_errno(code)),
        }
    }
}

/// Mounts the [`FuseDriver`] at `mountpoint` in the background with customizable mount options.
pub fn mount_virtual_fs_with_options(
    mut driver: FuseDriver,
    mountpoint: impl AsRef<Path>,
    options: Vec<MountOption>,
) -> Result<BackgroundSession, std::io::Error> {
    let mp = mountpoint.as_ref();
    if !mp.exists() {
        std::fs::create_dir_all(mp)?;
    }

    let mut config = Config::default();
    config.mount_options = options;
    // Two threads meant a single large hydration blocked every concurrent lookup/readdir, so the
    // mount looked hung while one image downloaded. Scale with the machine, within sane bounds.
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(4, 16);
    config.n_threads = Some(threads);
    config.clone_fd = true;

    // Keep payload downloads strictly below the thread-pool size so a burst of them can never
    // occupy every worker: metadata (`getattr`/`readdir`/`lookup`) then always has a thread free,
    // which is what keeps a directory listing responsive while photos download. With the clamp
    // above this leaves at least half the pool for metadata.
    let download_limit = (threads / 2).max(2);
    driver.set_download_limit(download_limit);
    debug!(
        threads,
        download_limit,
        "Bounding concurrent payload downloads below the FUSE thread-pool size"
    );

    debug!(mountpoint = %mp.display(), "Mounting virtual filesystem with FUSE");
    fuser::spawn_mount(driver, mp, &config)
}

/// Builds the mount options for the read-only virtual filesystem.
///
/// Deliberately does **not** include [`MountOption::AutoUnmount`]. `fuser` rejects that option
/// unless the session ACL is something other than [`SessionACL::Owner`]
/// (`fuser-0.18.0/src/session.rs:163`, "auto_unmount requires acl != Owner"), and both
/// non-owner ACLs map to `allow_other`, which `fusermount3` refuses unless the host has opted in
/// with `user_allow_other` in `/etc/fuse.conf`. Adding it back therefore makes every mount fail
/// outright on a stock host, so stale mounts from a crash are handled by `unmount_all_fuse`
/// instead.
#[must_use]
pub fn default_mount_options() -> Vec<MountOption> {
    vec![
        MountOption::FSName("Nextcloud".to_string()),
        MountOption::Subtype("nextcloud".to_string()),
        MountOption::RO,
        MountOption::NoAtime,
    ]
}

/// Mounts the [`FuseDriver`] at `mountpoint` in the background, returning a running session handle.
pub fn mount_virtual_fs(
    driver: FuseDriver,
    mountpoint: impl AsRef<Path>,
) -> Result<BackgroundSession, std::io::Error> {
    mount_virtual_fs_with_options(driver, mountpoint, default_mount_options())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuser::SessionACL;
    use std::time::SystemTime;

    #[test]
    fn attr_conversion_roundtrip() {
        let now = SystemTime::now();
        let attr = crate::attr::FileAttr::new(42, FileType::RegularFile, 1024, now);
        let fuser_attr: fuser::FileAttr = (&attr).into();
        assert_eq!(fuser_attr.ino.0, 42);
        assert_eq!(fuser_attr.size, 1024);
        assert_eq!(fuser_attr.kind, fuser::FileType::RegularFile);
    }

    /// Guards the mount options against the ACL precondition that silently broke every mount:
    /// `auto_unmount` is only legal with a non-`Owner` ACL, and this driver always uses the
    /// default (`Owner`) ACL.
    #[test]
    fn default_mount_options_are_compatible_with_owner_acl() {
        let options = default_mount_options();
        assert!(
            !options.contains(&MountOption::AutoUnmount),
            "AutoUnmount requires a non-Owner ACL; adding it makes every mount fail with \
             `auto_unmount requires acl != Owner`"
        );
        assert!(options.contains(&MountOption::RO));
        assert!(options.contains(&MountOption::NoAtime));

        // Mirror the check `fuser` performs in `Session::new`, so the invariant this crate
        // depends on is asserted against the real default rather than assumed.
        let mut config = Config::default();
        config.mount_options = options;
        assert_eq!(config.acl, SessionACL::Owner);
        assert!(
            !(config.mount_options.contains(&MountOption::AutoUnmount)
                && config.acl == SessionACL::Owner),
            "this option/ACL combination is rejected by fuser::Session::new"
        );
    }

    #[test]
    fn errno_mapping() {
        assert_eq!(to_errno(2), Errno::ENOENT);
        assert_eq!(to_errno(22), Errno::EINVAL);
        assert_eq!(to_errno(20), Errno::ENOTDIR);
        assert_eq!(to_errno(34), Errno::ERANGE);
        assert_eq!(to_errno(999), Errno::EIO);
    }
}
