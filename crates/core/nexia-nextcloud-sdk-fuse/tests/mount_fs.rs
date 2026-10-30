//! End-to-end tests that mount a real FUSE filesystem and read it back through the kernel.
//!
//! Everything here goes through the ordinary VFS: `read` and `readdir` exercise the kernel, the
//! FUSE driver and the hydrator together, which is the only way to catch the failures that unit
//! tests on `FuseDriver` structurally cannot see — mount option rejection, `fsname`/`subtype`
//! metadata, and read semantics (`EIO` versus a silently short read).
//!
//! Linux only. A host without `/dev/fuse` (most containers, CI sandboxes) does not fail the
//! suite: each test reports a skip instead, so the tests stay useful locally and honest in CI.
//!
//! Every mount goes through [`MountedFs`], which owns the tokio runtime as well as the session.
//! That is not incidental: `FuseDriver` resolves its runtime handle at construction time and, with
//! no handle, every FUSE read returns `EIO`. A bare `#[test]` has no runtime, so a driver built
//! without one fails with `EIO` for a reason that has nothing to do with hydration.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// `FuseDriver` takes a `std::sync::RwLock`, not the async one: the FUSE callback thread must be
/// able to lock the tree without an executor.
use std::sync::RwLock;

use nexia_nextcloud_sdk_fuse::{
    AsyncHydrator, BackgroundSession, FuseDriver, MountOption, mount_virtual_fs,
    mount_virtual_fs_with_options,
};
use nexia_nextcloud_sdk_vfs::{NodeMetadata, VfsError, VirtualNode, VirtualTree};

type HydratorFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + 'a>>;

const ENTRY_TIMEOUT: Duration = Duration::from_secs(10);

/// Owns a scratch directory that is removed on drop, so a failed assertion cannot leave a
/// mounted or half-created tree behind.
struct TempMountpoint(PathBuf);

impl TempMountpoint {
    fn new(label: &str) -> Self {
        let unique = format!(
            "nexia-fuse-test-{label}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("create mountpoint");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempMountpoint {
    fn drop(&mut self) {
        // Unmount before the directory disappears; a still-mounted path cannot be removed and the
        // kernel would keep answering `ENOTCONN` from a stale entry.
        let _ = std::process::Command::new("fusermount3")
            .args(["-u", "-z", &self.0.to_string_lossy()])
            .status();
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An active mount plus the runtime keeping its hydrator serviceable.
///
/// Field order is load-bearing: Rust drops fields in declaration order, so the FUSE session is
/// unmounted (and the runtime is still alive to service any in-flight hydration) before the
/// mountpoint directory is cleaned up.
struct MountedFs {
    session: BackgroundSession,
    runtime: tokio::runtime::Runtime,
    mp: TempMountpoint,
}

impl MountedFs {
    fn path(&self) -> &Path {
        self.mp.path()
    }

    fn entry(&self, name: &str) -> PathBuf {
        self.mp.path().join(name)
    }

    /// Waits for the mounted entry to become visible, then returns its path.
    fn wait_for_entry(&self, name: &str) -> PathBuf {
        let path = self.entry(name);
        assert!(
            wait_until(ENTRY_TIMEOUT, || path.exists()),
            "the mounted entry never appeared at {}",
            path.display()
        );
        path
    }

    /// Returns the `/proc/self/mountinfo` line describing this mount, panicking if absent.
    fn mountinfo_line(&self) -> String {
        let needle = self.mp.path().to_string_lossy().to_string();
        let info = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
        info.lines()
            .find(|line| line.contains(&needle))
            .unwrap_or_else(|| panic!("no mountinfo entry for {needle}"))
            .to_string()
    }
}

/// Returns `true` when this host can actually mount a FUSE filesystem.
fn fuse_available() -> bool {
    Path::new("/dev/fuse").exists()
        && (Path::new("/usr/bin/fusermount3").exists()
            || Path::new("/bin/fusermount3").exists()
            || Path::new("/usr/bin/fusermount").exists())
}

macro_rules! require_fuse {
    () => {
        if !fuse_available() {
            eprintln!(
                "skipping {}: no usable /dev/fuse or fusermount3 on this host",
                module_path!()
            );
            return;
        }
    };
}

/// Waits until `condition` holds, polling because the FUSE session runs on its own threads.
fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    condition()
}

/// Serves a fixed payload, counting how often it is actually asked for bytes.
struct CountingHydrator {
    payload: Vec<u8>,
    full_calls: AtomicUsize,
    range_calls: AtomicUsize,
    fail: AtomicBool,
}

impl CountingHydrator {
    fn new(payload: &[u8]) -> Arc<Self> {
        Arc::new(Self {
            payload: payload.to_vec(),
            full_calls: AtomicUsize::new(0),
            range_calls: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
        })
    }

    fn calls(&self) -> usize {
        self.full_calls.load(Ordering::SeqCst) + self.range_calls.load(Ordering::SeqCst)
    }
}

impl AsyncHydrator for CountingHydrator {
    fn hydrate(&self, _canonical_path: &str) -> HydratorFuture<'_> {
        self.full_calls.fetch_add(1, Ordering::SeqCst);
        let fail = self.fail.load(Ordering::SeqCst);
        let payload = self.payload.clone();
        Box::pin(async move {
            if fail {
                return Err(VfsError::NotFound("simulated fetch failure".to_string()));
            }
            Ok(payload)
        })
    }

    fn hydrate_range(&self, _canonical_path: &str, offset: u64, size: u32) -> HydratorFuture<'_> {
        self.range_calls.fetch_add(1, Ordering::SeqCst);
        let fail = self.fail.load(Ordering::SeqCst);
        let payload = self.payload.clone();
        Box::pin(async move {
            if fail {
                return Err(VfsError::NotFound("simulated fetch failure".to_string()));
            }
            let start = (offset as usize).min(payload.len());
            let end = (start + size as usize).min(payload.len());
            Ok(payload[start..end].to_vec())
        })
    }
}

/// Builds a driver over `tree` wired to a live runtime, which is what the desktop does in
/// `crates/nexia-sync/src/fuse_mount.rs` before mounting.
fn driver_with(
    runtime: &tokio::runtime::Runtime,
    tree: VirtualTree,
    hydrator: Arc<dyn AsyncHydrator>,
) -> FuseDriver {
    FuseDriver::with_hydrator(Arc::new(RwLock::new(tree)), hydrator)
        .with_runtime(runtime.handle().clone())
}

/// Mounts `tree` with the shipped default options and returns the live mount.
fn mount_with_defaults(
    label: &str,
    tree: VirtualTree,
    hydrator: Arc<dyn AsyncHydrator>,
) -> MountedFs {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build test runtime");
    let driver = driver_with(&runtime, tree, hydrator);
    let created = TempMountpoint::new(label);
    let session = mount_virtual_fs(driver, created.path())
        .expect("mount_virtual_fs must accept the default option set");
    MountedFs {
        session,
        runtime,
        mp: created,
    }
}

/// Builds a one-file tree at `/photo.bin` with the given size in metadata.
fn single_file_tree(size: u64) -> VirtualTree {
    let mut tree = VirtualTree::new("Nextcloud");
    tree.insert(VirtualNode::new_file(
        NodeMetadata::file("/photo.bin").with_size(size),
    ));
    tree
}

/// A deterministic payload that is not accidentally all-zero, so short reads are visible.
fn patterned_payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// The bug that took every mount down: `AutoUnmount` is rejected by `fuser` unless the session
/// ACL is non-`Owner`, and this driver always uses the default `Owner` ACL. Mounting must succeed
/// with the shipped options, and the mount must actually serve file content.
#[test]
fn default_options_mount_succeeds_and_serves_reads() {
    require_fuse!();

    let payload = patterned_payload(4096);
    let hydrator = CountingHydrator::new(&payload);
    let mounted = mount_with_defaults(
        "default-options",
        single_file_tree(payload.len() as u64),
        hydrator,
    );

    let file = mounted.wait_for_entry("photo.bin");
    let bytes = std::fs::read(&file).expect("read through the mount");
    assert_eq!(
        bytes, payload,
        "content read back through FUSE must match the payload"
    );
}

/// `fsname`/`subtype` are what `mount` and desktop tooling report; a missing or misapplied option
/// set is invisible to a same-process `read` test but obvious here.
#[test]
fn mount_options_surface_in_self_reported_fsinfo() {
    require_fuse!();

    let mounted = mount_with_defaults(
        "fsinfo",
        single_file_tree(5),
        CountingHydrator::new(b"hello"),
    );
    mounted.wait_for_entry("photo.bin");

    let line = mounted.mountinfo_line();
    assert!(
        line.contains("fuse.nextcloud"),
        "expected a fuse.nextcloud subtype in mountinfo, got: {line}"
    );
    assert!(
        line.contains("Nextcloud"),
        "expected the Nextcloud fsname in mountinfo, got: {line}"
    );
    assert!(
        line.contains("ro"),
        "expected a read-only mount in mountinfo, got: {line}"
    );
}

/// A caller-supplied option vector must be applied verbatim, not silently replaced.
#[test]
fn explicit_options_are_honoured() {
    require_fuse!();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build test runtime");
    let driver = driver_with(&runtime, single_file_tree(1), CountingHydrator::new(b"x"));
    let created = TempMountpoint::new("explicit");
    let options = vec![
        MountOption::FSName("NexiaTestFsName".to_string()),
        MountOption::Subtype("nexiatest".to_string()),
        MountOption::RO,
    ];
    let session = mount_virtual_fs_with_options(driver, created.path(), options)
        .expect("mount with explicit options");
    let mounted = MountedFs {
        session,
        runtime,
        mp: created,
    };

    mounted.wait_for_entry("photo.bin");
    let line = mounted.mountinfo_line();
    assert!(
        line.contains("fuse.nexiatest"),
        "explicit subtype was not applied, mountinfo line: {line}"
    );
    assert!(
        line.contains("NexiaTestFsName"),
        "explicit fsname was not applied, mountinfo line: {line}"
    );
}

/// A placeholder read must reach the hydrator and return the full payload, not zero bytes and not
/// a fabricated `EOF`.
#[test]
fn placeholder_read_hydrates_through_the_mount() {
    require_fuse!();

    let payload = patterned_payload(300_000);
    let hydrator = CountingHydrator::new(&payload);
    let mounted = mount_with_defaults(
        "hydrate",
        single_file_tree(payload.len() as u64),
        hydrator.clone(),
    );

    let file = mounted.wait_for_entry("photo.bin");
    let bytes = std::fs::read(&file).expect("read through the mount");
    assert_eq!(bytes.len(), payload.len(), "short read through FUSE");
    assert_eq!(bytes, payload, "hydrated content must match the payload");
    assert!(
        hydrator.calls() > 0,
        "the hydrator was never consulted for a placeholder read"
    );
}

/// A failing fetch must surface as an I/O error, never as a zero-length successful read — that
/// silent `EOF` is what made image viewers show a broken/empty file instead of retrying.
#[test]
fn failed_hydration_surfaces_eio_not_short_read() {
    require_fuse!();

    let hydrator = CountingHydrator::new(b"irrelevant");
    hydrator.fail.store(true, Ordering::SeqCst);
    let mounted = mount_with_defaults("eio", single_file_tree(64), hydrator.clone());

    let file = mounted.wait_for_entry("photo.bin");
    let err = std::fs::read(&file).expect_err("a failing hydrator must not yield a successful read");
    assert!(
        !matches!(err.kind(), std::io::ErrorKind::UnexpectedEof),
        "failure must be reported as EIO, not EOF: {err:?}"
    );
    assert!(
        hydrator.calls() > 0,
        "the read failed before the hydrator was ever consulted, so EIO came from the wrong place"
    );
}

/// Directory enumeration through the kernel: names, and no phantom entries.
#[test]
fn readdir_lists_children_through_the_mount() {
    require_fuse!();

    let mut tree = VirtualTree::new("Nextcloud");
    for name in ["alpha.txt", "beta.txt", "gamma.bin"] {
        tree.insert(VirtualNode::new_file(
            NodeMetadata::file(format!("/{name}")).with_size(1),
        ));
    }

    let mounted = mount_with_defaults("readdir", tree, CountingHydrator::new(b"x"));
    mounted.wait_for_entry("alpha.txt");

    let mut names: Vec<String> = std::fs::read_dir(mounted.path())
        .expect("readdir through the mount")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();

    assert_eq!(names, vec!["alpha.txt", "beta.txt", "gamma.bin"]);
}

/// The mount is read-only by contract; a write attempt must be refused rather than accepted.
#[test]
fn mount_is_read_only() {
    require_fuse!();

    let mounted = mount_with_defaults("readonly", single_file_tree(1), CountingHydrator::new(b"x"));
    mounted.wait_for_entry("photo.bin");

    let err = std::fs::write(mounted.entry("new.txt"), b"nope")
        .expect_err("writing to a read-only mount must fail");
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::ReadOnlyFilesystem | std::io::ErrorKind::PermissionDenied
        ),
        "expected a read-only refusal, got {err:?}"
    );
}

/// An empty tree is what a not-yet-synced account looks like; it must mount and enumerate as an
/// empty directory rather than being rejected.
#[test]
fn empty_tree_mounts_and_enumerates_empty() {
    require_fuse!();

    let mounted = mount_with_defaults(
        "empty",
        VirtualTree::new("Nextcloud"),
        CountingHydrator::new(b""),
    );

    let is_listable = wait_until(ENTRY_TIMEOUT, || {
        std::fs::read_dir(mounted.path()).is_ok()
    });
    assert!(is_listable, "readdir on an empty mount must succeed");

    let count = std::fs::read_dir(mounted.path())
        .expect("readdir through the mount")
        .filter_map(Result::ok)
        .count();
    assert_eq!(count, 0, "expected an empty directory, got {count} entries");
}

/// Dropping the session unmounts; the path becomes a plain directory again and the file is gone.
#[test]
fn dropping_the_session_unmounts_the_path() {
    require_fuse!();

    let mounted = mount_with_defaults("unmount", single_file_tree(1), CountingHydrator::new(b"x"));
    let file = mounted.wait_for_entry("photo.bin");

    let MountedFs { session, runtime, mp } = mounted;
    drop(session);

    assert!(
        wait_until(ENTRY_TIMEOUT, || !file.exists()),
        "the mount survived dropping its session handle"
    );

    let needle = mp.path().to_string_lossy().to_string();
    let info = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    assert!(
        !info.contains(&needle),
        "the unmounted path is still listed in /proc/self/mountinfo"
    );

    // An unmounted mountpoint must be an ordinary removable directory again.
    drop(runtime);
    drop(mp);
}

/// Mounting a second filesystem over an active mountpoint must never silently take it over: the
/// first session keeps serving, and the second attempt is refused.
#[test]
fn remounting_the_same_path_does_not_clobber_the_first_mount() {
    require_fuse!();

    let payload = patterned_payload(1024);
    let first = mount_with_defaults(
        "remount",
        single_file_tree(payload.len() as u64),
        CountingHydrator::new(&payload),
    );
    let file = first.wait_for_entry("photo.bin");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build test runtime");
    let second_driver = driver_with(&runtime, single_file_tree(1), CountingHydrator::new(b"y"));
    let second = mount_virtual_fs(second_driver, first.path());

    assert!(
        second.is_err(),
        "mounting over an active mountpoint reported success instead of being refused"
    );

    // Whatever the kernel decided, the original mount must still be the one answering.
    let bytes = std::fs::read(&file).expect("the first mount must still serve reads");
    assert_eq!(bytes, payload, "the original mount was clobbered");
}
