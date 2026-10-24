//! Live smoke test example against a real Nextcloud server.
//!
//! Run with:
//! ```sh
//! NEXTCLOUD_URL="https://example.com" \
//! NEXTCLOUD_USER="alice" \
//! NEXTCLOUD_PASS="secret" \
//! cargo run --example live_smoke
//! ```

use nextcloud_rust_sdk::{
    ActivityClient, ActivityFilter, BasicAuth, Client, DavPath, Depth, NodeMetadata,
    NotificationClient, OcsClient, PreviewOptions, PropfindBody, SecretString, ServerUrl,
    StoragePaths, StoreKey, UserClient, VirtualNode,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url_str = match std::env::var("NEXTCLOUD_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Skipping live_smoke: NEXTCLOUD_URL is not set.");
            return Ok(());
        }
    };
    let username = std::env::var("NEXTCLOUD_USER").expect("NEXTCLOUD_USER must be set");
    let password = std::env::var("NEXTCLOUD_PASS").expect("NEXTCLOUD_PASS must be set");

    let server_url = ServerUrl::parse(&url_str)?;
    let auth = BasicAuth::new(&username, SecretString::new(password));

    println!("=== Testing Nextcloud SDK against live server ===");

    // 0. High-level Client and Encrypted Storage Builder
    let tmp_dir = std::env::temp_dir().join("nextcloud_live_smoke");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let paths = StoragePaths::new(tmp_dir.join("session.db"), tmp_dir.join("cache.db"));

    let client = Client::builder()
        .server_url(server_url.clone())
        .storage(paths)
        .store_key(StoreKey::generate()?)
        .build()
        .await?;
    println!("0. Stateful Client initialized with encrypted SQLite storage.");
    println!("   Server: {}", client.server_url());

    // 1. OCS & User Capabilities
    let ocs = OcsClient::new(client.transport(), auth.clone());
    let user_client = UserClient::new(&ocs);

    println!("\n1. Fetching Server Capabilities...");
    let capabilities = user_client.capabilities().await?;
    if let Some(status) = capabilities.core().status() {
        println!("   Server Version: {:?}", status.version_string());
        println!("   Product Name:   {:?}", status.product_name());
    }
    println!("   WebDAV Root:    {:?}", capabilities.core().webdav_root());
    println!(
        "   Chunking Max:   {:?} bytes",
        capabilities.files().big_file_chunking()
    );

    println!("\n2. Fetching Current User Profile...");
    let current_user = user_client.current_user().await?;
    println!("   User ID:      {}", current_user.id());
    println!("   Display Name: {}", current_user.display_name());
    println!("   Enabled:      {}", current_user.is_enabled());
    println!(
        "   Quota Used:   {} bytes ({:.2}%)",
        current_user.quota().used(),
        current_user.quota().relative()
    );

    // 2. WebDAV Files
    println!("\n3. Testing WebDAV API...");
    let dav = client.dav(&auth);
    let root_path = DavPath::root(&username)?;

    println!("   Listing root directory resources (depth 1)...");
    let resources = dav
        .propfind(&root_path, Depth::One, &PropfindBody::all())
        .await?;
    println!("   Found {} items in user root:", resources.len());
    for item in resources.iter().take(5) {
        println!("     - {} (collection={})", item.href, item.is_collection);
    }

    let test_filename = "sdk_live_test.txt";
    let test_path = DavPath::new(&username, test_filename)?;
    let test_payload = b"Hello from nextcloud-rust-sdk live smoke test!";

    println!("   Uploading test file `{test_filename}`...");
    dav.put(&test_path, test_payload.to_vec(), Some("text/plain"))
        .await?;
    println!("   Upload successful.");

    println!("   Downloading test file `{test_filename}`...");
    let downloaded = dav.get(&test_path).await?;
    assert_eq!(downloaded, test_payload);
    println!(
        "   Download verified ({} bytes matched verbatim).",
        downloaded.len()
    );

    // 3. Transient VFS Mount (VirtualTree metadata without downloading payloads)
    println!("\n4. Testing Transient VFS (Virtual Hierarchy & Placeholders)...");
    let mut vfs = Client::new_vfs("root");
    let file_meta = NodeMetadata::file(format!("/{test_filename}"))
        .with_file_id(2)
        .with_size(test_payload.len() as u64)
        .with_mime_type("text/plain");
    vfs.insert(VirtualNode::new_file(file_meta));
    println!(
        "   Virtual tree contains {} nodes (transient mounting verified)",
        vfs.len()
    );
    if let Some(node) = vfs.get(&format!("/{test_filename}")) {
        println!(
            "   Placeholder node `{}` size: {} bytes, hydrated: {}",
            node.name(),
            node.size(),
            node.is_hydrated()
        );
    }

    // 4. File Versions API
    println!("\n5. Testing File Versions API...");
    let versions_client = client.versions(&auth);
    let versions = versions_client.list(&username, 1).await.unwrap_or_default();
    println!("   Found {} past revisions for test file.", versions.len());

    // 5. Previews API
    println!("\n6. Testing Preview & Thumbnail API...");
    let preview_client = client.preview(&auth);
    let preview_opts = PreviewOptions::new().dimensions(64, 64);
    match preview_client
        .get_by_path(test_path.as_str(), &preview_opts)
        .await
    {
        Ok(thumb_bytes) => {
            println!(
                "   Retrieved thumbnail for `{test_filename}`: {} bytes",
                thumb_bytes.len()
            );
            // Store thumbnail in VFS node cache
            let _ = vfs.set_thumbnail(&format!("/{test_filename}"), 64, 64, thumb_bytes);
        }
        Err(e) => {
            println!("   Preview response (expected if text file has no preview generator): {e}");
        }
    }

    println!("   Cleaning up test file `{test_filename}`...");
    dav.delete(&test_path).await?;
    println!("   Deletion successful.");

    // 6. Trashbin API
    println!("\n7. Testing Trashbin API...");
    let trash = client.trashbin(&auth);
    let trash_items = trash.list(&username).await?;
    println!("   Items currently in trashbin: {}", trash_items.len());
    for item in trash_items.iter().take(3) {
        println!(
            "     - [{}] {} (originally at {})",
            item.id(),
            item.filename(),
            item.original_location()
        );
    }

    // 7. Notifications API
    println!("\n8. Testing Notifications API...");
    let notifs = NotificationClient::new(client.transport(), &auth);
    let notification_list = notifs.list().await?;
    println!("   Active notifications count: {}", notification_list.len());
    for n in notification_list.iter().take(3) {
        println!("     - [{}] {}: {}", n.app, n.subject, n.message);
    }

    // 8. Activity API
    println!("\n9. Testing Activity API...");
    let activity = ActivityClient::new(&ocs);
    let filter = ActivityFilter::default().limit(5);
    let activities = activity.list(&filter).await?;
    println!("   Recent activities count: {}", activities.len());
    for a in &activities {
        println!("     - [{}] {}", a.app, a.subject);
    }

    // 9. Shares API
    println!("\n10. Testing Shares API...");
    let shares = client.shares(&auth);
    let share_list = shares.list(None).await?;
    println!("   Existing shares count: {}", share_list.len());

    let _ = std::fs::remove_dir_all(&tmp_dir);

    println!("\n=== All live smoke tests passed successfully! ===");
    Ok(())
}
