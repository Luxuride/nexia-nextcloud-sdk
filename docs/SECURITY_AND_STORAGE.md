# Security & Encrypted Storage Model

Security and data confidentiality are foundational tenets of the `nexia-nextcloud-sdk`. The SDK is engineered for zero credential leakage, compile-time memory safety, and military-grade encryption for local data at rest.

---

## 1. Encrypted SQLCipher Storage Architecture

The SDK separates mutable state into three distinct local SQLite databases, each
encrypted with [SQLCipher](https://www.zetetic.net/sqlcipher/) (AES-256) and each
opened through `EncryptedDatabase::open_credential_store(path, &key)`, which runs
`PRAGMA key` followed by a fail-fast `SELECT count(*) FROM sqlite_master`:

| Store | Contents |
| --- | --- |
| `auth.db` | accounts (server URL, login name, user id, app password blob) and login-flow state (poll endpoint, poll token) |
| `cache.db` | server capabilities JSON per server/user and `dav_entry` WebDAV listings (path, etag, size, content type, mtime) |
| `config.db` | settings key/value pairs, cached theme, stored sync mounts, per-directory sort order |

`auth.db` and `cache.db` are diagrammed below; `config.db` shares their layout and
teardown semantics (it can be rebuilt, at the cost of losing user preferences and
mount records).

```text
                   +---------------------------------------------+
                   |          Application Storage Root           |
                   |      (~/.local/share/nexia-nextcloud-sdk)    |
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

## 2. Multi-platform keyring integration (`nexia-nextcloud-sdk-keyring`)

Encryption keys and authentication tokens are never stored as plain files on disk. The SDK provides a unified cross-platform credential store abstraction via **`nexia-nextcloud-sdk-keyring`** through `KeychainKeyStore` and `PlatformKeyring`:

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

### 2.3 Lifecycle of key retrieval

```mermaid
sequenceDiagram
    participant App as Application
    participant Storage as nextcloud-sdk-storage
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

## 3. Zero credentials in Debug and logging

Accidental leakage of credentials into stdout, stderr, or system logs via formatted debug statements (`{:?}`) or log crates is a major security vulnerability in client applications. The Nextcloud Rust SDK implements strict redaction across every layer:

### 3.1 `SecretString`
Used for all sensitive passphrases, app passwords and bearer tokens. There are two:

| Type | Crate | `Debug` | Plaintext access |
| --- | --- | --- | --- |
| `nexia_nextcloud_auth::SecretString` | `nexia-nextcloud-auth` | `SecretString("<redacted>")` | `.expose() -> &str` |
| `nexia_nextcloud_sdk_keyring::SecretString` | `nexia-nextcloud-sdk-keyring` | `[REDACTED]` | `.expose_secret() -> &str`, `.into_inner() -> String` |

Both omit `Serialize`, so neither can be pushed into JSON logs. The keyring variant
also implements `Display` (redacted) and `Drop` — `Drop` zeroes the backing bytes
before releasing them.

### 3.2 Login Flow Token and Request Bodies
`nexia_nextcloud_sdk_session::PollToken` holds the token that collects the app
password the user is about to grant. Only `expose()` returns it; `Debug` renders
`PollToken(‹redacted›)` and the type implements no `Display`, so it cannot reach a
log through a derived `Debug` on `SessionState`, `SessionEvent` or `LoginFlow`.

`nexia_nextcloud_core::Request` prints `body_len: Option<usize>` instead of its
bytes, because a login or password-change body *is* the credential. Headers keep
the redaction described below.

### 3.3 HTTP Header Redaction
The `Header` type automatically detects sensitive HTTP header names (case-insensitively):
- Matching names: `authorization`, `proxy-authorization`, `cookie`, `set-cookie`.
- When formatted via `Debug`, the header value is replaced with `"<redacted>"`. Non-sensitive headers (e.g. `Content-Type: application/json`) remain readable for debugging.

### 3.4 User Profile Privacy Redaction
In `nexia-nextcloud-users`, the `User` struct contains sensitive personal metadata. Its `Debug` implementation redacts private attributes:
- `email`: Formatted as `email: Some("<redacted>")`.
- `storage_location`: Formatted as `storage_location: Some("<redacted>")` (prevents exposing internal server filesystem paths).

The redaction is a hand-written `impl fmt::Debug for User`
(`crates/nextcloud/nextcloud-users/src/user/profile.rs:149`), independent of the
`#[derive(Debug)]` on the wire struct it is built from.

### 3.4 Key & Database Redaction
- `StoreKey`: `Debug` writes `StoreKey(<redacted>)` (`crates/core/nextcloud-sdk-storage/src/key.rs:68`). The raw hex is never formatted.
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
  (Test modules and `#[cfg(test)]` code are exempt.)
- All fallible operations return strongly typed `Result<T, Error>`.
- Internal cause errors are preserved using standard Rust `#[source]` error chains, ensuring diagnostics can inspect the root cause without stringifying away error contexts.
