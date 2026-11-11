//! The platform bridges, one per target, selected by cfg here; a target with none, the web
//! included, runs on [`UnsupportedBackend`].

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_vendor = "apple")]
pub(crate) mod apple;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
mod unsupported;
#[cfg(target_os = "windows")]
pub(crate) mod windows;

pub use unsupported::UnsupportedBackend;

use std::sync::{Arc, OnceLock};

use crate::backend::Backend;

/// The backend of the target this build runs on: this target's bridge, or
/// [`UnsupportedBackend`] on a target with none, the web included.
pub(crate) fn default_backend() -> Arc<dyn Backend> {
    // The bridges import `LocalBackend`, which lib.rs compiles off wasm32; no wasm target
    // Rust ships matches a bridge cfg, and a custom one that did would fail on that import.
    #[cfg(target_vendor = "apple")]
    let backend = apple::NativeBackend::new();
    #[cfg(target_os = "android")]
    let backend = android::NativeBackend::new();
    #[cfg(target_os = "linux")]
    let backend = linux::NativeBackend::new();
    #[cfg(target_os = "windows")]
    let backend = windows::NativeBackend::new();
    #[cfg(not(any(
        target_vendor = "apple",
        target_os = "android",
        target_os = "linux",
        target_os = "windows"
    )))]
    let backend = UnsupportedBackend;
    Arc::new(backend)
}

/// The process's one backend, shared by every `BackgroundFetch::new()` and `Workmanager::new()`.
pub(crate) fn shared() -> Arc<dyn Backend> {
    static SHARED: OnceLock<Arc<dyn Backend>> = OnceLock::new();
    SHARED.get_or_init(default_backend).clone()
}
