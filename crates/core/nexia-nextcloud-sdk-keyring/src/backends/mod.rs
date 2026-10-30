//! Platform-specific backend implementations.

#[cfg(target_os = "linux")]
/// Linux Secret Service and Flatpak portal backend powered by `oo7`.
pub mod linux;

#[cfg(any(target_os = "macos", target_os = "windows"))]
/// Native desktop backend for macOS (Apple Keychain) and Windows (Credential Manager).
pub mod desktop;
