use super::*;

const CAPABILITIES_FULL: &str = r##"{
  "version": {
    "major": 30,
    "minor": 0,
    "micro": 4,
    "string": "30.0.4",
    "edition": "Community",
    "extendedSupport": false
  },
  "capabilities": {
    "core": {
      "pollinterval": 60,
      "webdav-root": "remote.php/webdav",
      "status": {
        "installed": true,
        "maintenance": false,
        "needsDbUpgrade": false,
        "version": "30.0.4.1",
        "versionstring": "30.0.4",
        "edition": "Community",
        "productname": "Nextcloud"
      }
    },
    "files": {
      "bigfilechunking": true,
      "undelete": true,
      "versioning": true,
      "chunked_upload": {
        "max_size": 104857600,
        "max_parallel_count": 5
      }
    },
    "theming": {
      "name": "Acme Cloud",
      "url": "https://cloud.acme.com",
      "slogan": "Secure Enterprise Cloud",
      "color": "#0082c9",
      "color-element": "#123456",
      "color-text": "#ffffff",
      "logo": "https://cloud.acme.com/apps/theming/logo.png",
      "background": "https://cloud.acme.com/apps/theming/bg.jpg",
      "background-plain": false,
      "background-default": true
    }
  }
}"##;

#[test]
fn deserializes_complete_capabilities_payload() {
    let caps: Capabilities = serde_json::from_str(CAPABILITIES_FULL).expect("valid capabilities");

    // Version assertions
    let version = caps.version().expect("version is present");
    assert_eq!(version.major(), 30);
    assert_eq!(version.minor(), Some(0));
    assert_eq!(version.micro(), Some(4));
    assert_eq!(version.string(), "30.0.4");
    assert_eq!(version.edition(), Some("Community"));
    assert_eq!(version.extended_support(), Some(false));
    assert_eq!(caps.server_version(), Some("30.0.4"));

    // Core capabilities assertions
    let core = caps.core();
    assert_eq!(core.poll_interval(), Some(60));
    assert_eq!(core.webdav_root(), Some("remote.php/webdav"));

    let status = core.status().expect("status is present");
    assert!(status.is_installed());
    assert!(!status.is_maintenance());
    assert!(!status.needs_db_upgrade());
    assert_eq!(status.version(), Some("30.0.4.1"));
    assert_eq!(status.version_string(), Some("30.0.4"));
    assert_eq!(status.edition(), Some("Community"));
    assert_eq!(status.product_name(), Some("Nextcloud"));

    // Files capabilities assertions
    let files = caps.files();
    assert!(files.big_file_chunking());
    assert!(files.undelete());
    assert!(files.versioning());
    assert_eq!(files.max_chunk_size(), Some(104857600));

    let chunked = files.chunked_upload().expect("chunked upload settings");
    assert_eq!(chunked.max_size(), Some(104857600));
    assert_eq!(chunked.max_parallel_count(), Some(5));

    // Theming capabilities assertions
    let theming = caps.theming().expect("theming settings");
    assert_eq!(theming.name(), Some("Acme Cloud"));
    assert_eq!(theming.url(), Some("https://cloud.acme.com"));
    assert_eq!(theming.slogan(), Some("Secure Enterprise Cloud"));
    assert_eq!(theming.color(), Some("#0082c9"));
    assert_eq!(theming.color_element(), Some("#123456"));
    assert_eq!(theming.color_text(), Some("#ffffff"));
    assert_eq!(
        theming.logo(),
        Some("https://cloud.acme.com/apps/theming/logo.png")
    );
    assert_eq!(
        theming.background(),
        Some("https://cloud.acme.com/apps/theming/bg.jpg")
    );
    assert!(!theming.is_background_plain());
    assert!(theming.is_background_default());
}

#[test]
fn deserializes_string_wire_types_tolerantly() {
    let raw = r#"{
      "capabilities": {
        "core": {
          "pollinterval": "120",
          "status": {
            "installed": "true",
            "maintenance": "0",
            "needsDbUpgrade": "false",
            "versionstring": "29.0.0"
          }
        },
        "files": {
          "bigfilechunking": "true",
          "undelete": "1",
          "versioning": "true",
          "max_chunk_size": "52428800"
        }
      }
    }"#;

    let caps: Capabilities = serde_json::from_str(raw).expect("tolerant parsing");
    assert_eq!(caps.core().poll_interval(), Some(120));
    assert!(caps.core().status().unwrap().is_installed());
    assert!(!caps.core().status().unwrap().is_maintenance());
    assert_eq!(caps.server_version(), Some("29.0.0"));
    assert!(caps.files().big_file_chunking());
    assert!(caps.files().undelete());
    assert!(caps.files().versioning());
    assert_eq!(caps.files().max_chunk_size(), Some(52428800));
}

#[test]
fn deserializes_minimal_empty_capabilities() {
    let raw = r#"{"capabilities": {}}"#;
    let caps: Capabilities = serde_json::from_str(raw).expect("empty capabilities ok");
    assert_eq!(caps.version(), None);
    assert_eq!(caps.server_version(), None);
    assert_eq!(caps.core().poll_interval(), None);
    assert_eq!(caps.core().webdav_root(), None);
    assert_eq!(caps.core().status(), None);
    assert!(!caps.files().big_file_chunking());
    assert!(!caps.files().undelete());
    assert!(!caps.files().versioning());
    assert_eq!(caps.theming(), None);
}
