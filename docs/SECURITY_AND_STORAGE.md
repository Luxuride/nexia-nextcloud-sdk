# Security & Encrypted Storage Model

Security and data confidentiality are foundational tenets of the `nextcloud-rust-sdk`. The SDK is engineered for zero credential leakage, compile-time memory safety, and military-grade encryption for local data at rest.

---

## 1. Dual Encrypted SQLCipher Storage Architecture

The SDK separates mutable state into two distinct local SQLite databases, each encrypted with [SQLCipher](https://www.zetetic.net/sqlcipher/) (AES-256 in CBC/GCM mode):

```
                   +---------------------------------------------+
                   |          Application Storage Root           |
                   |      (~/.local/share/nextcloud-rust-sdk)    |
                   +---------------------------------------------+
                                     /         \
                                    /           \
                                   v             v
             +---------------------------+   +---------------------------+
             |         auth.db           |   |         cache.db          |
             |  (CredentialSchema v1)    |   |     (CacheSchema v1)      |
             +---------------------------+   +---------------------------+
             | • account table           |   | • capabilities table      |
             |   - server URL            |   |   - server URL, user_id   |
             |   - login_name, user_id   |   |   - capabilities JSON blob|
             |   - app_password (BLOB)   |   |   - fetched_at timestamp  |
             | • login_flow table        |   | • dav_entry table         |
             |   - poll_endpoint         |   |   - path, etag, size      |
             |   - poll_token (BLOB)     |   |   - content_type, mtime   |
             |   - login_url             |   |   - fetched_at timestamp  |
             +---------------------------+   +---------------------------+
```

### 1.1 Separation of Concerns: `auth.db` vs `cache.db`
1. **`auth.db` (Credential Store)**:
   - Contains high-value authentication secrets, including App Passwords, active Login Flow v2 polling tokens, and account identifiers.
   - Protected with rigorous zeroization policies and never mixed with transient server data.
   - If corrupted or purged, only authentication tokens are lost, prompting a standard re-login.
2. **`cache.db` (Cache Store)**:
   - Contains cached server capabilities (chunking limits, server version, theming) and WebDAV directory listings (`dav_entry`).
   - Enables instant offline UI rendering and reduces network roundtrips to the Nextcloud server.
   - Can be safely cleared or rebuilt at any time without invalidating user sessions.

### 1.2 Cryptographic Key Management: `StoreKey`
- **Full Entropy Generation**: Database encryption keys are generated via `StoreKey::generate()`, drawing 256 bits (32 bytes) of cryptographically secure random entropy from the OS random generator using `getrandom`.
- **Passphrase KDF Bypass**: Because the key is 256 bits of raw cryptographic entropy rather than a human-chosen passphrase, the key is installed directly using SQLCipher's raw hex pragma:
  ```sql
  PRAGMA key = "x'0123456789abcdef...'";
  ```
  This avoids the CPU-intensive PBKDF2 derivation step while guaranteeing maximum cryptographic strength.
- **Fail-Fast Open Verification**: Upon executing `PRAGMA key`, the database immediately executes `SELECT count(*) FROM sqlite_master`. If an incorrect key is supplied, SQLCipher fails immediately, producing `StorageError::KeyRejected` before any application query can execute.

---

## 2. Multi-Platform Keyring Integration (`sdk-keyring`)

Encryption keys and authentication tokens are never stored as plain files on disk. The SDK provides a unified cross-platform credential store abstraction via **`sdk-keyring`** through [`KeychainKeyStore`] and [`PlatformKeyring`]:

### 2.1 Supported Platform Backends
1. **Linux & Sandboxed Flatpak**:
   - Powered by **`oo7`** (`linux-credentials`).
   - On the host desktop: integrates seamlessly with D-Bus FreeDesktop Secret Service (`org.freedesktop.secrets`), including GNOME Keyring, KWallet, and KeePassXC.
   - Inside Flatpak containers: automatically uses the **XDG Desktop Secret Portal** (`org.freedesktop.portal.Secret`), allowing secure credential storage without punching holes in sandbox permissions.
2. **macOS**:
   - Integrates natively with **Apple Keychain Services** via `Security.framework`.
3. **Windows**:
   - Integrates natively with **Windows Credential Manager** (`wincred`).
4. **Headless / Testing**:
   - Dedicated [`MemoryKeyring`] for automated testing and CI runners without hardware keyrings.

### 2.2 Schema Attributes and Isolation
Keys are cataloged under the application's unique reverse-DNS identifier and account label:

| Attribute | Value / Example | Description |
| :--- | :--- | :--- |
| `xdg:schema` | `dev.luxuride.NextcloudSdk.StoreKey` | Secret Service schema identifier |
| `application`| `org.example.NextcloudApp` | Enclosing application ID |
| `account`    | `default` / `user@example.org` | Account-specific database namespace |

This design prevents multiple Nextcloud accounts or different client applications from interfering with or overwriting each other's database keys.

### 2.2 Lifecycle of Key Retrieval

```mermaid
sequenceDiagram
    participant App as Application
    participant Storage as sdk-storage
    participant oo7 as oo7 (Secret Service)
    participant Keyring as Linux Keyring / GNOME Keyring
    participant DB as SQLCipher auth.db

    App->>Storage: StoragePaths::default_xdg()
    App->>Storage: KeychainKeyStore::new(app_id, account)
    App->>Storage: load_or_create()
    Storage->>oo7: Keyring::new().search_items(attributes)
    oo7->>Keyring: Search schema 'dev.luxuride.NextcloudSdk.StoreKey'
    alt Key Exists
        Keyring-->>oo7: Return encrypted Secret
        oo7-->>Storage: Hex key material
        Storage-->>App: StoreKey
    else Key Not Found
        Storage->>Storage: StoreKey::generate() (256-bit OS entropy)
        Storage->>oo7: Keyring::create_item(label, attributes, secret)
        oo7->>Keyring: Persist in Linux Keyring
        Storage-->>App: StoreKey
    end
    App->>DB: EncryptedDatabase::open_credential_store(path, &key)
    DB->>DB: PRAGMA key = "x'...'"; SELECT count(*) FROM sqlite_master;
    DB-->>App: Open Database Handle
```

---

## 3. Zero Credentials in Debug & Logging

Accidental leakage of credentials into stdout, stderr, or system logs via formatted debug statements (`{:?}`) or log crates is a major security vulnerability in client applications. The Nextcloud Rust SDK implements strict redaction across every layer:

### 3.1 `SecretString`
The `SecretString` type is used for all sensitive passphrases, app passwords, and bearer tokens:
- **Redacted Formatting**: The `Debug` implementation outputs `SecretString("<redacted>")` unconditionally.
- **No `Display` Impl**: `SecretString` deliberately omits `std::fmt::Display` to prevent accidental string interpolation (`println!("{password}")` does not compile).
- **No `Serialize` Impl**: Cannot be inadvertently serialized into public JSON logs.
- **Explicit Call Site Exposure**: The plaintext value is accessible exclusively via `.expose()`, making every access searchable and auditable across the codebase.

### 3.2 HTTP Header Redaction
The `Header` type automatically detects sensitive HTTP header names (case-insensitively):
- Matching names: `authorization`, `proxy-authorization`, `cookie`, `set-cookie`.
- When formatted via `Debug`, the header value is replaced with `"<redacted>"`. Non-sensitive headers (e.g. `Content-Type: application/json`) remain readable for debugging.

### 3.3 User Profile Privacy Redaction
In `nextcloud-users`, the `User` struct contains sensitive personal metadata. Its `Debug` implementation redacts private attributes:
- `email`: Formatted as `email: Some("<redacted>")`.
- `storage_location`: Formacted as `storage_location: Some("<redacted>")` (prevents exposing internal server filesystem paths).

### 3.4 Key & Database Redaction
- `StoreKey`: Formats as `StoreKey(<redacted>)`. The raw hex is never formatted.
- `KeychainKeyStore`: Debug output displays only the application ID and account label, omitting key contents.
- `EncryptedDatabase`: Formats only the file path, omitting encryption pragmas.

---

## 4. Memory Safety Guarantees

### 4.1 `#![forbid(unsafe_code)]`
The entire workspace is compiled with `#![forbid(unsafe_code)]` at the crate root of every member:
```rust
#![deny(missing_docs)]
#![forbid(unsafe_code)]
```
No `unsafe` blocks are permitted in any production code, library seam, or internal helper. All memory safety guarantees are mathematically verified by the Rust compiler.

### 4.2 Explicit Error Handling & Propagation
- No `panic!()`, `unwrap()`, or `expect()` on code paths reachable from library APIs.
- All fallible operations return strongly typed `Result<T, Error>`.
- Internal cause errors are preserved using standard Rust `#[source]` error chains, ensuring diagnostics can inspect the root cause without stringifying away error contexts.
