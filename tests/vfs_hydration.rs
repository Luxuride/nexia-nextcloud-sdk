//! Integration tests verifying virtual filesystem (VFS) metadata and on-demand hydration.
//!
//! Tests that unhydrated placeholders report true virtual sizes and modification times,
//! that read operations through the FUSE driver trigger transparent payload hydration,
//! and that the orchestrator accurately transitions file sync status between `OnlineOnly`
//! and `Synced`.

use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use nextcloud_rust_sdk::{
    AsyncHydrator, FuseDriver, MountDescriptor, MountMode, NextcloudOrchestrator, NodeMetadata,
    OrchestratorConfig, OrchestratorEvent, SyncFileStatus, VfsError, VirtualEntrySummary,
    VirtualNode, VirtualTree,
};

/// Mock hydrator that tracks hydration invocations and yields deterministic byte payloads.
struct MockHydrator {
    call_count: Arc<AtomicUsize>,
    payload: Vec<u8>,
}

impl MockHydrator {
    fn new(payload: Vec<u8>) -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        (
            Self {
                call_count: counter.clone(),
                payload,
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
        let data = self.payload.clone();
        Box::pin(async move { Ok(data) })
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
