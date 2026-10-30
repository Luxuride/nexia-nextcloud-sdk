use super::*;

const USER_CURRENT_JSON: &str = r#"{
  "enabled": true,
  "id": "alice",
  "display-name": "Alice Smith",
  "email": "alice@example.com",
  "storageLocation": "/var/www/nextcloud/data/alice",
  "backend": "Database",
  "groups": ["admin", "users"],
  "quota": {
    "free": 71373287424,
    "used": 694711965,
    "total": 72067999389,
    "relative": 0.96
  }
}"#;

const USER_PROVISIONING_JSON: &str = r#"{
  "enabled": true,
  "id": "bob",
  "displayname": "Bob Jones",
  "email": "bob@example.com",
  "home": "/var/www/nextcloud/data/bob",
  "backend": "LDAP",
  "groups": ["users"],
  "subadmin": ["department-a"],
  "quota": {
    "free": -3,
    "used": 1500000000,
    "total": -3,
    "relative": 0
  }
}"#;

#[test]
fn deserializes_current_user_profile() {
    let user: User = serde_json::from_str(USER_CURRENT_JSON).expect("valid current user");

    assert_eq!(user.id(), "alice");
    assert_eq!(user.display_name(), "Alice Smith");
    assert_eq!(user.email(), Some("alice@example.com"));
    assert!(user.is_enabled());
    assert_eq!(user.backend(), Some("Database"));
    assert_eq!(user.groups(), &["admin", "users"]);
    assert_eq!(
        user.storage_location(),
        Some("/var/www/nextcloud/data/alice")
    );

    let quota = user.quota();
    assert_eq!(quota.free(), 71373287424);
    assert_eq!(quota.used(), 694711965);
    assert_eq!(quota.total(), 72067999389);
    assert_eq!(quota.relative(), 0.96);
    assert!(!quota.is_unlimited());
    assert_eq!(quota.free_bytes(), Some(71373287424));
    assert_eq!(quota.total_bytes(), Some(72067999389));
    assert_eq!(quota.used_bytes(), 694711965);
    assert!((quota.relative_percentage() - 96.0).abs() < f64::EPSILON);
}

#[test]
fn deserializes_provisioned_user_with_unlimited_quota() {
    let user: User = serde_json::from_str(USER_PROVISIONING_JSON).expect("valid provisioned user");

    assert_eq!(user.id(), "bob");
    assert_eq!(user.display_name(), "Bob Jones");
    assert_eq!(user.email(), Some("bob@example.com"));
    assert!(user.is_enabled());
    assert_eq!(user.backend(), Some("LDAP"));
    assert_eq!(user.groups(), &["users"]);
    assert_eq!(user.subadmin(), &["department-a"]);
    assert_eq!(user.storage_location(), Some("/var/www/nextcloud/data/bob"));

    let quota = user.quota();
    assert!(quota.is_unlimited());
    assert_eq!(quota.free(), -3);
    assert_eq!(quota.total(), -3);
    assert_eq!(quota.free_bytes(), None);
    assert_eq!(quota.total_bytes(), None);
    assert_eq!(quota.used_bytes(), 1500000000);
}

#[test]
fn debug_formatting_redacts_sensitive_fields() {
    let user: User = serde_json::from_str(USER_CURRENT_JSON).expect("valid user");
    let debug_output = format!("{user:?}");

    // Non-sensitive fields should be visible
    assert!(debug_output.contains("alice"));
    assert!(debug_output.contains("Alice Smith"));
    assert!(debug_output.contains("Database"));

    // Sensitive fields (email, storage_location) MUST be redacted
    assert!(!debug_output.contains("alice@example.com"));
    assert!(!debug_output.contains("/var/www/nextcloud/data/alice"));
    assert!(debug_output.contains("<redacted>"));
}
