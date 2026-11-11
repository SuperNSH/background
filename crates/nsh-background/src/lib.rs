//! Background work for Native Shell apps: periodic fetch and deferred tasks, within the OS's
//! limits.
//!
//! [`BackgroundFetch`] follows `background_fetch`: configure a handler, start the periodic
//! fetch, schedule one-off tasks. [`Workmanager`] follows `workmanager`: register one-off and
//! periodic work by unique name, cancel it by name or tag. Each call resolves exactly once,
//! with a value or a [`BackgroundError`] carrying a stable code.

mod backend;
mod error;
mod fake;
#[cfg(not(target_arch = "wasm32"))]
mod local;
mod native;
mod options;
mod platform;
mod scheduler;
#[cfg(test)]
pub(crate) mod testing;
mod work;

use nsh_core::prelude::*;

pub use backend::{
    Backend, BackendFuture, CancelBy, TaskEvent, TaskHandler, WorkKind, WorkRequest,
};
pub use error::{BackgroundError, ErrorKind};
pub use fake::FakeBackend;
#[cfg(not(target_arch = "wasm32"))]
pub use local::{Limits, LocalBackend};
pub use native::{
    NativeBackgroundFetch, NativeFetchConfig, NativeOneOffOptions, NativePeriodicOptions,
    NativeTaskConfig, NativeWorkInfo,
};
pub use options::{
    BackoffPolicy, Constraints, ExistingPeriodicWorkPolicy, ExistingWorkPolicy, FetchConfig, Input,
    InputValue, NetworkType, Status, TaskConfig, TaskType, WorkInfo, WorkState,
};
pub use platform::UnsupportedBackend;
pub use scheduler::{BackgroundFetch, FETCH_TASK_ID};
pub use work::{OneOffOptions, PeriodicOptions, Workmanager};

/// The crate's UI set: what TSX imports from `nsh-background/native`.
#[native_module("nsh-background/native")]
fn native(registry: &mut Registry) {
    registry.class::<NativeBackgroundFetch>();
}
