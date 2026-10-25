use nextcloud_sdk_orchestrator::{
    MountDescriptor, MountMode, NextcloudOrchestrator, OrchestratorConfig, OrchestratorEvent,
    SyncFileStatus,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("nextcloud-orch-test-{pid}-{counter}-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn test_orchestrator_multi_mount_and_sync_status() {
    let temp = TestDir::new("main");
    let cache_dir = temp.path().join("cache");
    let target_root = temp.path().join("target_root");
    let target_sub = temp.path().join("target_sub");

    std::fs::create_dir_all(&target_root).unwrap();
    std::fs::create_dir_all(&target_sub).unwrap();

    let config = OrchestratorConfig::new(&cache_dir);
    let orchestrator = NextcloudOrchestrator::start(config).unwrap();
    let handle = orchestrator.handle();
    let mut event_rx = handle.subscribe_events();

    // 1. Add root mount: "/" -> target_root
    let root_mount_desc = MountDescriptor::new("acc-1", "/", &target_root, MountMode::VirtualVfs)
        .with_display_name("Nextcloud Root");
    let root_mount_id = handle.mounts().add_mount(root_mount_desc).await.unwrap();

    // 2. Add subdirectory mount: "/Photos" -> target_sub
    let sub_mount_desc =
        MountDescriptor::new("acc-1", "/Photos", &target_sub, MountMode::VirtualVfs)
            .with_display_name("Nextcloud Photos");
    let sub_mount_id = handle.mounts().add_mount(sub_mount_desc).await.unwrap();

    // Verify both mounts are listed
    let mounts = handle.mounts().list_mounts().await.unwrap();
    assert_eq!(mounts.len(), 2);

    // Verify events were emitted
    let mut added_events = 0;
    while let Ok(evt) = event_rx.try_recv() {
        if let OrchestratorEvent::MountAdded(_) = evt {
            added_events += 1;
        }
    }
    assert!(added_events >= 2);

    // 3. Test File Sync Status and Icons
    // Default status for un-hydrated file is OnlineOnly (cloud icon)
    let status_default = handle
        .mounts()
        .file_status(root_mount_id.clone(), "Documents/readme.txt")
        .await
        .unwrap();
    assert_eq!(status_default, SyncFileStatus::OnlineOnly);
    assert_eq!(status_default.icon_name(), "cloud-symbolic");
    assert_eq!(status_default.emblem_name(), "folder-remote-symbolic");
    assert!(!status_default.is_locally_available());
    assert!(status_default.is_online_only());

    // 4. Test VFS operations through handle
    let vfs = handle.vfs(root_mount_id.clone());
    vfs.create_directory("/Documents").await.unwrap();

    // Hydrate local file
    let content = b"Hello from Nextcloud SDK Orchestrator!";
    let local_path = vfs
        .hydrate_local("/Documents/readme.txt", content.to_vec())
        .await
        .unwrap();
    assert!(local_path.exists());
    assert_eq!(std::fs::read(&local_path).unwrap(), content);

    // After hydration, file status is Synced (ok icon)
    let status_hydrated = handle
        .mounts()
        .file_status(root_mount_id.clone(), "Documents/readme.txt")
        .await
        .unwrap();
    assert_eq!(status_hydrated, SyncFileStatus::Synced);
    assert_eq!(status_hydrated.icon_name(), "emblem-ok-symbolic");
    assert_eq!(status_hydrated.emblem_name(), "emblem-default-symbolic");
    assert!(status_hydrated.is_locally_available());
    assert!(!status_hydrated.is_online_only());

    // Dehydrate file
    vfs.dehydrate_local("/Documents/readme.txt").await.unwrap();
    assert!(!local_path.exists());

    // After dehydration, file status reverts to OnlineOnly
    let status_dehydrated = handle
        .mounts()
        .file_status(root_mount_id.clone(), "Documents/readme.txt")
        .await
        .unwrap();
    assert_eq!(status_dehydrated, SyncFileStatus::OnlineOnly);

    // 5. Query status by local filesystem path inside target directory
    handle
        .mounts()
        .set_file_status(sub_mount_id.clone(), "holiday.jpg", SyncFileStatus::Syncing)
        .await
        .unwrap();

    let local_img_path = target_sub.join("holiday.jpg");
    let status_by_path = handle
        .mounts()
        .status_for_local_path(&local_img_path)
        .await
        .unwrap();
    assert_eq!(status_by_path, Some(SyncFileStatus::Syncing));
    if let Some(s) = status_by_path {
        assert_eq!(s.icon_name(), "emblem-synchronizing-symbolic");
        assert_eq!(s.emblem_name(), "view-refresh-symbolic");
    }

    // 6. Shutdown orchestrator
    handle.shutdown().await.unwrap();
}
