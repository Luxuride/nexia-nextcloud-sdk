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
    ActivityClient, ActivityFilter, BasicAuth, DavClient, DavPath, Depth, NotificationClient,
    OcsClient, PropfindBody, SecretString, ServerUrl, ShareClient, TransportBuilder, UserClient,
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

    let server_url = ServerUrl::parse(url_str)?;
    let transport = TransportBuilder::with_server(server_url).build()?;
    let auth = BasicAuth::new(&username, SecretString::new(password));

    println!("=== Testing Nextcloud SDK against live server ===");

    // 1. OCS & User Capabilities
    let ocs = OcsClient::new(&transport, auth.clone());
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
    let dav = DavClient::new(&transport, &auth);
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

    println!("   Cleaning up test file `{test_filename}`...");
    dav.delete(&test_path).await?;
    println!("   Deletion successful.");

    // 3. Notifications API
    println!("\n4. Testing Notifications API...");
    let notifs = NotificationClient::new(&transport, &auth);
    let notification_list = notifs.list().await?;
    println!("   Active notifications count: {}", notification_list.len());
    for n in notification_list.iter().take(3) {
        println!("     - [{}] {}: {}", n.app, n.subject, n.message);
    }

    // 4. Activity API
    println!("\n5. Testing Activity API...");
    let activity = ActivityClient::new(&ocs);
    let filter = ActivityFilter::default().limit(5);
    let activities = activity.list(&filter).await?;
    println!("   Recent activities count: {}", activities.len());
    for a in &activities {
        println!("     - [{}] {}", a.app, a.subject);
    }

    // 5. Shares API
    println!("\n6. Testing Shares API...");
    let shares = ShareClient::new(&transport, &auth);
    let share_list = shares.list(None).await?;
    println!("   Existing shares count: {}", share_list.len());

    println!("\n=== All live smoke tests passed successfully! ===");
    Ok(())
}
