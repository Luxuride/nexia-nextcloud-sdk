//! Integration tests verifying virtual filesystem (VFS) metadata and on-demand hydration.
//!
//! Tests that unhydrated placeholders report true virtual sizes and modification times,
//! that read operations through the FUSE driver trigger transparent payload hydration,
//! that thumbnailers are prevented from triggering mass hydration, that FreeDesktop fail
//! markers are managed correctly, and that recursive directory hydration and dehydration
//! work accurately.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use nextcloud_rust_sdk::{
    AsyncHydrator, FuseDriver, MountDescriptor, MountMode, NextcloudOrchestrator, NodeMetadata,
    OrchestratorConfig, OrchestratorEvent, SyncFileStatus, VfsError, VirtualEntrySummary,
    VirtualNode, VirtualTree, freedesktop_thumbnail_dirs, install_freedesktop_fail_marker,
    install_freedesktop_thumbnail_with_mtime, md5_hex, path_to_file_uri,
    remove_freedesktop_fail_marker,
};

/// Mock hydrator that tracks hydration invocations and yields deterministic byte payloads.
struct MockHydrator {
    call_count: Arc<AtomicUsize>,
    payload: Result<Vec<u8>, VfsError>,
}

impl MockHydrator {
    fn new(payload: Vec<u8>) -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            Self {
                call_count: counter.clone(),
                payload: Ok(payload),
            },
            counter,
        )
    }

    fn failing(error: VfsError) -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            Self {
                call_count: counter.clone(),
                payload: Err(error),
            },
            counter,
        )
    }
}

impl AsyncHydrator for MockHydrator {
    fn hydrate(
        &self,
        _canonical_path: &str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, VfsError>> + Send + '_>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let res = self.payload.clone();
        Box::pin(async move { res })
    }
}

fn temp_cache_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vfs-hydration-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[test]
fn placeholder_preserves_virtual_metadata_before_hydration() {
    let mut tree = VirtualTree::new("root");
    let remote_size = 24_576_000u64; // ~24.5 MB photo
    let remote_mtime = "Tue, 15 Aug 2025 15:30:00 GMT";

    let file_meta = NodeMetadata::file("/Camera/20250815_153000.jpg")
        .with_size(remote_size)
        .with_modified_at(remote_mtime);

    let node = VirtualNode::new_file(file_meta);
    assert!(
        !node.is_hydrated(),
        "Placeholder node must initially be unhydrated"
    );
    assert_eq!(
        node.size(),
        remote_size,
        "Unhydrated node must expose its virtual size"
    );

    tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
        "/Camera",
    )));
    tree.insert(node);

    let found = tree
        .get("/Camera/20250815_153000.jpg")
        .expect("Node must exist in tree");
    assert!(
        !found.is_hydrated(),
        "Tree lookup must confirm node is unhydrated"
    );
    assert_eq!(
        found.size(),
        remote_size,
        "Tree lookup must report true remote size"
    );

    let summary = VirtualEntrySummary::from(found);
    assert!(!summary.is_hydrated);
    assert_eq!(summary.size, remote_size);
    assert_eq!(summary.modified_at.as_deref(), Some(remote_mtime));

    // Summary of directory must show it contains unhydrated items
    let dir_summary = tree.directory_size_summary("/Camera");
    assert!(
        dir_summary.has_unhydrated,
        "Directory must register as having unhydrated files"
    );
}

#[tokio::test]
async fn fuse_driver_transparently_hydrates_on_read() {
    let mut tree = VirtualTree::new("root");
    let test_payload = b"IMG_RAW_DATA_PAYLOAD_BYTES_NEXTCLOUD_VFS".to_vec();
    let payload_len = test_payload.len() as u32;

    let file_meta = NodeMetadata::file("/test_image.jpg").with_size(payload_len as u64);
    tree.insert(VirtualNode::new_file(file_meta));

    let (hydrator, call_count) = MockHydrator::new(test_payload.clone());
    let shared_tree = Arc::new(RwLock::new(tree));
    let driver = FuseDriver::with_hydrator(shared_tree.clone(), Arc::new(hydrator));

    // Lookup should succeed and expose correct virtual size before any read/hydration occurs
    let root_inode = 1;
    let attr = driver
        .lookup(root_inode, "test_image.jpg")
        .expect("Lookup must find test_image.jpg");
    assert!(attr.is_file());
    assert_eq!(
        attr.size, payload_len as u64,
        "Virtual metadata size must match remote length"
    );
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "No hydration before read"
    );

    // Verify driver identifies node as an unhydrated placeholder
    assert!(
        driver.is_placeholder(attr.ino),
        "Driver must recognize unhydrated node as placeholder"
    );

    // First read triggers transparent hydration
    let read_chunk = driver
        .read(attr.ino, 0, payload_len)
        .await
        .expect("Read must succeed");
    assert_eq!(
        read_chunk, test_payload,
        "Read payload must match hydrator output"
    );
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Hydrator must be called exactly once"
    );

    // Verify tree state transitioned to hydrated
    {
        let tree_guard = shared_tree.read().unwrap();
        let node = tree_guard.get("/test_image.jpg").expect("Node must exist");
        assert!(
            node.is_hydrated(),
            "Node must now be marked as hydrated in VirtualTree"
        );
    }

    // Driver must now report is_placeholder as false
    assert!(
        !driver.is_placeholder(attr.ino),
        "Driver must recognize hydrated node is no longer placeholder"
    );

    // Subsequent read must be served from cache without calling hydrator again
    let cached_chunk = driver
        .read(attr.ino, 0, 8)
        .await
        .expect("Subsequent read must succeed");
    assert_eq!(cached_chunk, &test_payload[..8]);
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Cached read must not re-invoke hydrator"
    );
}

#[test]
fn freedesktop_fail_marker_lifecycle() {
    let test_file = Path::new("/home/user/Nextcloud/Photos/vacation.jpg");
    let uri = path_to_file_uri(test_file);
    let hash = md5_hex(uri.as_bytes());
    let filename = format!("{hash}.png");

    let (normal_dir, large_dir) = freedesktop_thumbnail_dirs();
    let fail_file = normal_dir
        .parent()
        .map(|p| {
            p.join("fail")
                .join("gnome-thumbnail-factory")
                .join(&filename)
        })
        .expect("fail file path");

    // 1. Install fail marker when placeholder is created
    install_freedesktop_fail_marker(test_file, 1700000000);
    assert!(
        fail_file.exists(),
        "Fail marker must exist to prevent Nautilus thumbnailer invocation"
    );

    // 2. Installing genuine thumbnail from cloud preview removes fail marker
    let minimal_png = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    install_freedesktop_thumbnail_with_mtime(test_file, &minimal_png, 1700000000);

    assert!(
        !fail_file.exists(),
        "Installing genuine thumbnail must clear fail marker"
    );
    assert!(
        normal_dir.join(&filename).exists(),
        "Normal thumbnail must be installed"
    );
    assert!(
        large_dir.join(&filename).exists(),
        "Large thumbnail must be installed"
    );

    // Cleanup
    let _ = std::fs::remove_file(normal_dir.join(&filename));
    let _ = std::fs::remove_file(large_dir.join(&filename));
    remove_freedesktop_fail_marker(test_file);
}

#[tokio::test]
async fn orchestrator_hydrates_dehydrates_and_updates_status() {
    let cache_dir = temp_cache_dir("orchestrator-status");
    let target_dir = temp_cache_dir("target");
    let config = OrchestratorConfig::new(&cache_dir);

    let orchestrator = NextcloudOrchestrator::start(config).expect("Orchestrator must start");
    let handle = orchestrator.handle();
    let mut event_rx = handle.subscribe_events();

    let mount_desc = MountDescriptor::new("acc-test", "/", &target_dir, MountMode::VirtualVfs);
    let mount_id = handle
        .mounts()
        .add_mount(mount_desc)
        .await
        .expect("Add mount");

    let remote_file = "Documents/readme.txt";
    let file_payload = b"Hello from Nextcloud SDK Orchestrator Hydration Test!".to_vec();

    let vfs = handle.vfs(mount_id.clone());
    vfs.create_directory("/Documents")
        .await
        .expect("Create directory");

    // Initially, file status must be OnlineOnly (unhydrated placeholder)
    let initial_status = handle
        .mounts()
        .file_status(mount_id.clone(), remote_file)
        .await
        .expect("Status check");
    assert_eq!(initial_status, SyncFileStatus::OnlineOnly);
    assert!(!initial_status.is_locally_available());
    assert!(initial_status.is_online_only());

    // Hydrate local file
    let local_path = vfs
        .hydrate_local("/Documents/readme.txt", file_payload.clone())
        .await
        .expect("Hydrate local");
    assert!(local_path.exists(), "Hydrated file must exist on disk");
    assert_eq!(
        std::fs::read(&local_path).expect("Read cached file"),
        file_payload,
        "Cached file bytes must match hydrated content"
    );

    // Verify OrchestratorEvent::FileHydrated was emitted
    let mut hydrated_event_received = false;
    while let Ok(event) = event_rx.try_recv() {
        if let OrchestratorEvent::FileHydrated {
            ref remote_path, ..
        } = event
            && remote_path.contains("readme.txt")
        {
            hydrated_event_received = true;
            break;
        }
    }
    assert!(
        hydrated_event_received,
        "FileHydrated event must be broadcast"
    );

    // Status must now transition to Synced (locally available)
    let hydrated_status = handle
        .mounts()
        .file_status(mount_id.clone(), remote_file)
        .await
        .expect("Status check");
    assert_eq!(hydrated_status, SyncFileStatus::Synced);
    assert!(hydrated_status.is_locally_available());
    assert!(!hydrated_status.is_online_only());

    // Dehydrate file back to placeholder
    vfs.dehydrate_local("/Documents/readme.txt")
        .await
        .expect("Dehydrate local");
    assert!(
        !local_path.exists(),
        "Local cached file must be removed upon dehydration"
    );

    // Verify OrchestratorEvent::FileDehydrated was emitted
    let mut dehydrated_event_received = false;
    while let Ok(event) = event_rx.try_recv() {
        if let OrchestratorEvent::FileDehydrated {
            ref remote_path, ..
        } = event
            && remote_path.contains("readme.txt")
        {
            dehydrated_event_received = true;
            break;
        }
    }
    assert!(
        dehydrated_event_received,
        "FileDehydrated event must be broadcast"
    );

    // Status must transition back to OnlineOnly
    let dehydrated_status = handle
        .mounts()
        .file_status(mount_id, remote_file)
        .await
        .expect("Status check");
    assert_eq!(dehydrated_status, SyncFileStatus::OnlineOnly);
    assert!(!dehydrated_status.is_locally_available());
    assert!(dehydrated_status.is_online_only());
}

#[tokio::test]
async fn directory_recursive_hydration_and_dehydration() {
    let mut tree = VirtualTree::new("root");
    tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
        "/Photos",
    )));
    tree.insert(VirtualNode::new_directory(NodeMetadata::directory(
        "/Photos/2026",
    )));

    let f1_data = b"PHOTO_1_BYTES".to_vec();
    let f2_data = b"PHOTO_2_BYTES".to_vec();

    tree.insert(VirtualNode::new_file(
        NodeMetadata::file("/Photos/2026/img1.jpg").with_size(f1_data.len() as u64),
    ));
    tree.insert(VirtualNode::new_file(
        NodeMetadata::file("/Photos/2026/img2.jpg").with_size(f2_data.len() as u64),
    ));

    // Summary must report has_unhydrated
    let summary_before = tree.directory_size_summary("/Photos");
    assert!(
        summary_before.has_unhydrated,
        "Directory must initially contain unhydrated files"
    );
    assert_eq!(summary_before.file_count, 2);

    // Hydrate all files under /Photos
    tree.hydrate("/Photos/2026/img1.jpg", f1_data.clone())
        .expect("hydrate img1");
    tree.hydrate("/Photos/2026/img2.jpg", f2_data.clone())
        .expect("hydrate img2");

    let summary_after = tree.directory_size_summary("/Photos");
    assert!(
        !summary_after.has_unhydrated,
        "Directory must not have unhydrated files after full hydration"
    );
    assert_eq!(summary_after.file_count, 2);
    assert_eq!(
        summary_after.total_size,
        (f1_data.len() + f2_data.len()) as u64
    );

    // Dehydrate one file
    tree.dehydrate("/Photos/2026/img1.jpg")
        .expect("dehydrate img1");

    let summary_partial = tree.directory_size_summary("/Photos");
    assert!(
        summary_partial.has_unhydrated,
        "Directory must report unhydrated files after one is dehydrated"
    );
    assert_eq!(summary_partial.file_count, 2);
}

#[tokio::test]
async fn concurrent_hydration_deduplication() {
    let mut tree = VirtualTree::new("root");
    let test_payload = b"CONCURRENT_SHARED_PAYLOAD_BYTES".to_vec();
    let file_meta = NodeMetadata::file("/concurrent.bin").with_size(test_payload.len() as u64);
    tree.insert(VirtualNode::new_file(file_meta));

    let (hydrator, call_count) = MockHydrator::new(test_payload.clone());
    let shared_tree = Arc::new(RwLock::new(tree));
    let driver = Arc::new(FuseDriver::with_hydrator(
        shared_tree.clone(),
        Arc::new(hydrator),
    ));

    let attr = driver.lookup(1, "concurrent.bin").expect("lookup file");

    // Spawn 10 concurrent read tasks for the same unhydrated file
    let mut handles = Vec::new();
    for _ in 0..10 {
        let d = driver.clone();
        let payload_len = test_payload.len() as u32;
        let ino = attr.ino;
        handles.push(tokio::spawn(
            async move { d.read(ino, 0, payload_len).await },
        ));
    }

    for h in handles {
        let res = h.await.expect("task join").expect("read ok");
        assert_eq!(res, test_payload, "All concurrent tasks receive valid data");
    }

    // Hydrator should have been called only once
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "Hydration must be deduplicated across concurrent readers"
    );
}

#[tokio::test]
async fn hydration_failure_preserves_placeholder_integrity() {
    let mut tree = VirtualTree::new("root");
    let remote_size = 10_000u64;
    let file_meta = NodeMetadata::file("/broken.bin").with_size(remote_size);
    tree.insert(VirtualNode::new_file(file_meta));

    let (hydrator, call_count) =
        MockHydrator::failing(VfsError::NotFound("/broken.bin".to_string()));
    let shared_tree = Arc::new(RwLock::new(tree));
    let driver = FuseDriver::with_hydrator(shared_tree.clone(), Arc::new(hydrator));

    let attr = driver.lookup(1, "broken.bin").expect("lookup file");
    assert_eq!(attr.size, remote_size);

    // Read should fail with ENOENT (2)
    let res = driver.read(attr.ino, 0, 1024).await;
    assert!(res.is_err(), "Read must fail when hydrator fails");
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Verify tree node remains an unhydrated placeholder with intact virtual size
    {
        let tree_guard = shared_tree.read().unwrap();
        let node = tree_guard.get("/broken.bin").expect("Node exists");
        assert!(
            !node.is_hydrated(),
            "Node must remain unhydrated after failed read"
        );
        assert_eq!(
            node.size(),
            remote_size,
            "Virtual size must not be clobbered by failure"
        );
    }
}

#[test]
fn vfs_directory_preservation_and_recovery() {
    let temp_root = temp_cache_dir("vfs-dir-recovery");
    let local_sync_root = temp_root.join("local");
    let journal_path = temp_root.join("sync.journal");
    std::fs::create_dir_all(&local_sync_root).unwrap();

    let runner = nextcloud_rust_sdk::FolderSyncRunner::new(&local_sync_root, "/", &journal_path);

    // 1. Remote has a directory /Camera
    let remote_camera = nextcloud_rust_sdk::DiscoveredRemoteFile {
        relative_path: "/Camera".to_string(),
        href: "/remote.php/dav/files/user/Camera/".to_string(),
        size: 1048576,
        etag: Some("\"etag-cam-1\"".to_string()),
        is_directory: true,
    };

    // 2. Scenario: A 0-byte file was mistakenly created on disk for /Camera (from previous bug)
    let camera_on_disk = local_sync_root.join("Camera");
    std::fs::write(&camera_on_disk, b"").unwrap();
    assert!(camera_on_disk.is_file());

    // Reconciliation in VFS mode:
    let (actions, _) = runner
        .plan_reconciliation(std::slice::from_ref(&remote_camera), true)
        .expect("plan reconciliation");

    // Must NEVER emit Download for a directory
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, nextcloud_rust_sdk::SyncAction::Download { .. })),
        "Directory must NEVER be scheduled for Download: {actions:?}"
    );

    // 3. Testing apply_placeholder refuses to overwrite a directory
    let real_dir = local_sync_root.join("RealDir");
    std::fs::create_dir_all(&real_dir).unwrap();
    let mut journal = nextcloud_rust_sdk::SyncJournal::new();
    let err = runner.apply_placeholder("/RealDir", 100, None, &mut journal);
    assert!(err.is_err(), "apply_placeholder must fail on a directory");

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_root);
}
