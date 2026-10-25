# Getting Started with the Nextcloud Rust SDK

This guide provides a comprehensive tutorial for integrating the `nextcloud-rust-sdk` into your Rust application. It covers storage initialization, hardware/keychain-backed key management, authentication with Nextcloud's Login Flow v2, and executing API calls across WebDAV, capabilities, user profiles, shares, notifications, and activity feeds.

---

## 1. Adding Dependencies

Add the SDK and Tokio runtime dependencies to your `Cargo.toml`:

```toml
[dependencies]
nextcloud-rust-sdk = "0.1.2"
tokio = { version = "1", features = ["full"] }
```

If you only require specific stateless subsets (e.g., WebDAV without storage or local state machines), you can depend directly on individual workspace crates:

```toml
[dependencies]
nextcloud-sdk-core = "0.1.2"
nextcloud-auth = "0.1.2"
nextcloud-transport = "0.1.2"
nextcloud-dav = "0.1.2"
```

---

## 2. Initializing Encrypted Storage & Keys

Nextcloud Rust SDK stores account credentials and local cache data in two separate encrypted SQLCipher databases:
- `auth.db`: Stores account logins, app passwords, and active Login Flow v2 polling sessions.
- `cache.db`: Caches server capabilities and WebDAV file hierarchies for responsive offline operation.

The 256-bit AES encryption key (`StoreKey`) is never stored on disk in plaintext. On Linux systems, it is securely stored and retrieved from the FreeDesktop Secret Service / GNOME Keyring / KWallet using the **`oo7`** crate via `KeychainKeyStore`.

```rust
use nextcloud_rust_sdk::{
    EncryptedDatabase, KeychainKeyStore, KeyStore, StoragePaths, StoreKey,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Resolve storage paths (defaults to ~/.local/share/nextcloud-rust-sdk/)
    let paths = StoragePaths::default_xdg()?;
    paths.ensure_directories()?;

    // 2. Load or generate the 256-bit database key via the Linux Secret Service (oo7)
    let keystore = KeychainKeyStore::new("org.example.NextcloudApp", "default_account");
    let key: StoreKey = keystore.load_or_create().await?;

    // 3. Open the encrypted SQLCipher databases
    let auth_db: EncryptedDatabase = paths.open_auth_db(&key)?;
    let cache_db: EncryptedDatabase = paths.open_cache_db(&key)?;

    println!("Encrypted storage ready at: {:?}", paths.auth());
    Ok(())
}
```

---

## 3. Direct Transport & Client Assembly

You can build a transport connected to your Nextcloud instance using `TransportBuilder`:

```rust
use nextcloud_rust_sdk::{
    BasicAuth, ReqwestTransport, SecretString, ServerUrl, TimeoutConfig, TransportBuilder,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_url = ServerUrl::parse("https://cloud.example.org")?;

    // Configure transport with custom network timeouts
    let transport: ReqwestTransport = TransportBuilder::with_server(server_url)
        .timeouts(
            TimeoutConfig::default()
                .with_connect_timeout(Duration::from_secs(10))
                .with_read_timeout(Duration::from_secs(30)),
        )
        .build()?;

    // Initialize credentials
    let auth = BasicAuth::new("alice", SecretString::new("app-password-here"));

    Ok(())
}
```

---

## 4. Nextcloud Login Flow v2

Nextcloud's Login Flow v2 allows desktop and mobile applications to authenticate without ever asking users for their master password or handling 2FA tokens directly. Instead, authorization takes place in the user's web browser, yielding an isolated App Password.

The SDK's pure state machine `SessionMachine` models this lifecycle without performing direct I/O:

```
[Idle] ──> SessionEvent::StartRequested ──> [AwaitingLogin] (Browser opened)
   │
   └──> SessionEvent::PollPending ──> [AwaitingGrant] (Polling 404 / 200)
           │
           └──> SessionEvent::Granted ──> [Authenticated] (App Password stored)
```

### Complete Login Flow Walkthrough

```rust
use nextcloud_rust_sdk::{
    Account, ApiPath, Command, HttpMethod, LoginStart, LoginSuccess, PollTarget, PollToken,
    Request, ReqwestTransport, SecretString, ServerUrl, SessionEvent, SessionMachine,
    SessionState, Transport, TransportBuilder,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct FlowStartResponse {
    poll: PollEndpoint,
    login: String,
}

#[derive(Deserialize)]
struct PollEndpoint {
    token: String,
    endpoint: String,
}

#[derive(Deserialize)]
struct PollSuccessResponse {
    #[serde(rename = "server")]
    server: String,
    #[serde(rename = "loginName")]
    login_name: String,
    #[serde(rename = "appPassword")]
    app_password: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_url = ServerUrl::parse("https://cloud.example.org")?;
    let transport = TransportBuilder::with_server(server_url.clone()).build()?;
    let mut session = SessionMachine::new();

    // 1. Transition state machine to initiate login
    let command = session.handle(SessionEvent::StartRequested)?;
    assert_eq!(command, Command::StartLogin);

    // 2. Call Nextcloud's /login/v2 endpoint to initialize the flow
    let init_path = ApiPath::parse("index.php/login/v2")?;
    let init_req = Request::post(init_path);
    let init_res = transport.execute(init_req).await?;
    let start_data: FlowStartResponse = serde_json::from_slice(init_res.body())?;

    // 3. Supply flow start parameters to the session machine
    let start_event = LoginStart::new(
        &start_data.login,
        &start_data.poll.endpoint,
        PollToken::new(&start_data.poll.token),
    );
    let command = session.handle(SessionEvent::FlowStarted(start_event))?;

    // 4. Instruct user or system browser to open the authorization URL
    if let Command::OpenBrowser(login_url) = command {
        println!("Please authenticate in your browser: {login_url}");
        // Example: open::that(&login_url)?;
    }

    // 5. Poll the Nextcloud server until the user approves access
    let mut authenticated_account: Option<Account> = None;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let poll_cmd = session.handle(SessionEvent::PollPending)?;
        let Command::Poll(target) = poll_cmd else {
            break;
        };

        // Send POST request to poll endpoint with the token
        let poll_path = ApiPath::parse(&target.url)?;
        let poll_req = Request::post(poll_path)
            .body_bytes(format!("token={}", target.token).into_bytes());

        let res = transport.execute(poll_req).await?;
        if res.status().as_u16() == 404 {
            // Still waiting for user approval in the browser
            continue;
        }

        if res.status().is_success() {
            let poll_result: PollSuccessResponse = serde_json::from_slice(res.body())?;
            let success = LoginSuccess::new(
                ServerUrl::parse(poll_result.server)?,
                poll_result.login_name,
                SecretString::new(poll_result.app_password),
            );

            let next_cmd = session.handle(SessionEvent::Granted(success))?;
            if let Command::Establish(account) = next_cmd {
                authenticated_account = Some(*account);
                println!("Successfully authenticated!");
                break;
            }
        }
    }

    Ok(())
}
```

---

## 5. Interacting with Nextcloud APIs

Once authenticated, use the strongly typed subclients for high-level operations.

### 5.1 WebDAV Files & Folders (`DavClient`)

The `nextcloud-dav` crate provides file manipulation across Nextcloud's WebDAV endpoint (`/remote.php/dav/files/{user}/{path}`):

```rust
use nextcloud_rust_sdk::{
    BasicAuth, DavClient, DavPath, Depth, PropfindBody, ReqwestTransport, SecretString,
    ServerUrl, TransportBuilder,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TransportBuilder::with_server(ServerUrl::parse("https://cloud.example.org")?).build()?;
    let auth = BasicAuth::new("alice", SecretString::new("app-password"));
    let dav = DavClient::new(&transport, &auth);

    let user = "alice";
    let root = DavPath::root(user)?;

    // 1. List directory entries with PROPFIND
    let resources = dav.propfind(&root, Depth::One, &PropfindBody::all()).await?;
    for item in resources {
        println!("Resource: {} (is_dir: {})", item.href, item.is_collection);
    }

    // 2. Upload a file with PUT
    let file_path = DavPath::new(user, "Documents/notes.txt")?;
    dav.put(&file_path, b"Hello Nextcloud!".to_vec(), Some("text/plain")).await?;

    // 3. Download a file with GET
    let contents = dav.get(&file_path).await?;
    println!("Downloaded: {}", String::from_utf8_lossy(&contents));

    // 4. Create directory with MKCOL
    let dir_path = DavPath::new(user, "Documents/Archive")?;
    dav.mkcol(&dir_path).await?;

    // 5. Delete a file or directory
    dav.delete(&file_path).await?;

    Ok(())
}
```

### 5.2 Server Capabilities & User Profile (`UserClient`)

Query server version, chunking limits, and user storage quotas:

```rust
use nextcloud_rust_sdk::{
    BasicAuth, OcsClient, ReqwestTransport, SecretString, ServerUrl, TransportBuilder,
    UserClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TransportBuilder::with_server(ServerUrl::parse("https://cloud.example.org")?).build()?;
    let auth = BasicAuth::new("alice", SecretString::new("app-password"));
    let ocs = OcsClient::new(&transport, auth);
    let user_client = UserClient::new(&ocs);

    // Fetch capabilities
    let caps = user_client.capabilities().await?;
    if let Some(status) = caps.core().status() {
        println!("Server version: {:?}", status.version_string());
    }
    println!("WebDAV root: {:?}", caps.core().webdav_root());
    println!("Max chunk size: {:?} bytes", caps.files().big_file_chunking());

    // Fetch current user details & quota
    let me = user_client.current_user().await?;
    println!("User: {} ({})", me.display_name(), me.id());
    println!("Quota used: {} bytes ({:.2}%)", me.quota().used(), me.quota().relative());

    Ok(())
}
```

### 5.3 OCS File Sharing (`ShareClient`)

Create public links, share with other users, and configure share permissions:

```rust
use nextcloud_rust_sdk::{
    BasicAuth, CreateShare, ReqwestTransport, SecretString, ServerUrl, ShareClient,
    SharePermissions, TransportBuilder,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TransportBuilder::with_server(ServerUrl::parse("https://cloud.example.org")?).build()?;
    let auth = BasicAuth::new("alice", SecretString::new("app-password"));
    let shares = ShareClient::new(&transport, &auth);

    // Create a read-only public link share
    let req = CreateShare::public_link("/Documents/presentation.pdf")
        .with_permissions(SharePermissions::READ);

    let share = shares.create(&req).await?;
    println!("Share ID: {}", share.id());
    if let Some(url) = share.url() {
        println!("Public Link URL: {url}");
    }

    // List all existing shares
    let all_shares = shares.list(None).await?;
    println!("Total shares: {}", all_shares.len());

    // Delete the share
    shares.delete(share.id()).await?;

    Ok(())
}
```

### 5.4 Notifications (`NotificationClient`)

Fetch and dismiss user notifications:

```rust
use nextcloud_rust_sdk::{
    BasicAuth, NotificationClient, ReqwestTransport, SecretString, ServerUrl,
    TransportBuilder,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TransportBuilder::with_server(ServerUrl::parse("https://cloud.example.org")?).build()?;
    let auth = BasicAuth::new("alice", SecretString::new("app-password"));
    let notifs = NotificationClient::new(&transport, &auth);

    // List notifications
    let list = notifs.list().await?;
    for n in list {
        println!("[{}] {}: {}", n.app, n.subject, n.message);
        // Dismiss notification
        notifs.delete(n.notification_id).await?;
    }

    Ok(())
}
```

### 5.5 Activity Streams (`ActivityClient`)

Fetch audit logs and activity streams:

```rust
use nextcloud_rust_sdk::{
    ActivityClient, ActivityFilter, BasicAuth, OcsClient, ReqwestTransport,
    SecretString, ServerUrl, TransportBuilder,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = TransportBuilder::with_server(ServerUrl::parse("https://cloud.example.org")?).build()?;
    let auth = BasicAuth::new("alice", SecretString::new("app-password"));
    let ocs = OcsClient::new(&transport, auth);
    let activity = ActivityClient::new(&ocs);

    // Query 10 most recent activities
    let filter = ActivityFilter::new().limit(10);
    let items = activity.list(&filter).await?;

    for item in items {
        println!("Activity: [{}] {} (by {})", item.app, item.subject, item.user);
    }

    Ok(())
}
```

### 5.6 Bandwidth Rate Limiting & Download Progress Reporting

Configure upload and download bandwidth limits via [`ClientBuilder`], listen to real-time transfer progress, and dynamically adjust limits without rebuilding the client:

```rust
use nextcloud_rust_sdk::{BandwidthLimit, Client, ServerUrl, StoragePaths, StoreKey};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_url = ServerUrl::parse("https://cloud.example.org")?;
    let paths = StoragePaths::default_xdg()?;
    let key = StoreKey::generate()?;

    // 1. Configure initial rate limits and transfer progress callbacks in the builder
    let client = Client::builder()
        .server_url(server_url)
        .storage(paths)
        .store_key(key)
        .upload_limit(BandwidthLimit::from_mib(2))    // Cap uploads at 2 MiB/s
        .download_limit(BandwidthLimit::from_mib(10)) // Cap downloads at 10 MiB/s
        .on_download_progress(|progress| {
            if let Some(pct) = progress.percentage() {
                println!("Download: {:.1}% ({}/{} bytes)", pct, progress.transferred_bytes(), progress.total_bytes().unwrap_or(0));
            } else {
                println!("Downloaded: {} bytes", progress.transferred_bytes());
            }
        })
        .build()
        .await?;

    // 2. Adjust bandwidth limits dynamically on the running client (e.g. from user GUI settings)
    client.set_download_limit(BandwidthLimit::from_kib(500)); // Throttle to 500 KiB/s
    client.set_upload_limit(BandwidthLimit::unlimited());     // Remove upload restriction

    println!("Active download limit: {}", client.download_limit());
    Ok(())
}
```
