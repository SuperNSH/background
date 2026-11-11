//! The Android bridge: the in-process engine under a 15-minute floor, WorkManager's own
//! minimum for periodic work.

use std::time::Duration;

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskHandler, WorkRequest},
    error::BackgroundError,
    local::{Limits, LocalBackend},
    options::{FetchConfig, Status, TaskConfig, WorkInfo},
};

/// The Android backend of v0.1: [`LocalBackend`] under WorkManager's own minimum for
/// periodic work.
///
/// `force_alarm_manager`, `start_on_boot` and `stop_on_terminate` are accepted and stored
/// only; there is no OS scheduler to hand them to yet (see `docs/parity.md`).
pub(crate) struct NativeBackend(LocalBackend);

impl NativeBackend {
    /// The engine with WorkManager's 15-minute floor under the fetch interval.
    pub(crate) fn new() -> Self {
        Self(LocalBackend::new(Limits {
            min_fetch_interval: Duration::from_secs(15 * 60),
        }))
    }
}

impl Backend for NativeBackend {
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.0.status()
    }

    fn configure(
        &self,
        config: FetchConfig,
        handler: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.0.configure(config, handler)
    }

    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.0.start()
    }

    fn stop(&self, task_id: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.0.stop(task_id)
    }

    fn schedule_task(
        &self,
        config: TaskConfig,
    ) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.0.schedule_task(config)
    }

    fn finish(&self, task_id: &str) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.0.finish(task_id)
    }

    fn register(&self, request: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.0.register(request)
    }

    fn is_scheduled(&self, unique_name: &str) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.0.is_scheduled(unique_name)
    }

    fn work_info(
        &self,
        unique_name: &str,
    ) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>> {
        self.0.work_info(unique_name)
    }

    fn cancel(&self, cancel_by: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.0.cancel(cancel_by)
    }
}
