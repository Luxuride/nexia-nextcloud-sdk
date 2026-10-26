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
            blksize: 512,
        }
    }
}

fn to_errno(code: i32) -> Errno {
    match code {
        2 => Errno::ENOENT,
        22 => Errno::EINVAL,
        20 => Errno::ENOTDIR,
        34 => Errno::ERANGE,
        _ => Errno::EIO,
    }
}

fn is_thumbnailer(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check /proc/{pid}/comm
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
            let name = comm.trim();
            if name.contains("thumbnail") || name == "tumblerd" {
                return true;
            }
        }
        // Check /proc/{pid}/cmdline
        if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline")) {
            let cmd_str = String::from_utf8_lossy(&cmdline);
            if cmd_str.contains("thumbnail")
                || cmd_str.contains("tumblerd")
                || cmd_str.contains("glycin-thumbnailer")
            {
                return true;
            }
        }
        // Check parent process (e.g. if sandboxed inside bwrap)
        if let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
            for line in status.lines() {
                if let Some(ppid_str) = line.strip_prefix("PPid:")
                    && let Ok(ppid) = ppid_str.trim().parse::<u32>()
                    && ppid > 1
                {
                    if let Ok(pcomm) = std::fs::read_to_string(format!("/proc/{ppid}/comm")) {
                        let pname = pcomm.trim();
                        if pname.contains("thumbnail") || pname == "tumblerd" {
                            return true;
                        }
                    }
                    if let Ok(pcmdline) = std::fs::read(format!("/proc/{ppid}/cmdline")) {
                        let pcmd_str = String::from_utf8_lossy(&pcmdline);
                        if pcmd_str.contains("thumbnail")
                            || pcmd_str.contains("tumblerd")
                            || pcmd_str.contains("glycin-thumbnailer")
                        {
                            return true;
                        }
                    }
                    break;
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
    }
    false
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
        reply.statfs(1_000_000, 500_000, 500_000, 1_000_000, 500_000, 512, 255, 0);
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
        reply.opened(FileHandle(ino.0), FopenFlags::empty());
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

        if is_thumbnailer(req.pid()) && self.is_placeholder(ino.0) {
            debug!(
                pid = req.pid(),
                ino = ino.0,
                "Declined thumbnailer read on unhydrated placeholder to prevent mass hydration"
            );
            reply.error(Errno::ENODATA);
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
    driver: FuseDriver,
    mountpoint: impl AsRef<Path>,
    options: Vec<MountOption>,
) -> Result<BackgroundSession, std::io::Error> {
    let mp = mountpoint.as_ref();
    if !mp.exists() {
        std::fs::create_dir_all(mp)?;
    }

    let mut config = Config::default();
    config.mount_options = options;
    config.n_threads = Some(2);

    debug!(mountpoint = %mp.display(), "Mounting virtual filesystem with FUSE");
    fuser::spawn_mount(driver, mp, &config)
}

/// Mounts the [`FuseDriver`] at `mountpoint` in the background, returning a running session handle.
pub fn mount_virtual_fs(
    driver: FuseDriver,
    mountpoint: impl AsRef<Path>,
) -> Result<BackgroundSession, std::io::Error> {
    let options = vec![
        MountOption::FSName("Nextcloud".to_string()),
        MountOption::Subtype("nextcloud-vfs".to_string()),
        MountOption::RO,
    ];
    mount_virtual_fs_with_options(driver, mountpoint, options)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn errno_mapping() {
        assert_eq!(to_errno(2), Errno::ENOENT);
        assert_eq!(to_errno(22), Errno::EINVAL);
        assert_eq!(to_errno(20), Errno::ENOTDIR);
        assert_eq!(to_errno(34), Errno::ERANGE);
        assert_eq!(to_errno(999), Errno::EIO);
    }
}
