//! The Windows bridge: the local engine under default limits, because a desktop app owns its
//! background threads. A WinRT `BackgroundTaskBuilder` bridge is pending.

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskHandler, WorkRequest},
    error::BackgroundError,
    local::{Limits, LocalBackend},
    options::{FetchConfig, Status, TaskConfig, WorkInfo},
};

/// The Windows backend of v0.1: every call delegates to a [`LocalBackend`] with no interval
/// floor. A WinRT `BackgroundTaskBuilder` bridge is pending.
pub(crate) struct NativeBackend {
    inner: LocalBackend,
}

impl NativeBackend {
    /// The backend with no floor under the fetch interval: the app owns its threads.
    pub(crate) fn new() -> Self {
        Self {
            inner: LocalBackend::new(Limits::default()),
        }
    }
}

impl Backend for NativeBackend {
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.inner.status()
    }

    fn configure(
        &self,
        config: FetchConfig,
        handler: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.inner.configure(config, handler)
    }

    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        self.inner.start()
    }

    fn stop(&self, task_id: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.inner.stop(task_id)
    }

    fn schedule_task(
        &self,
        config: TaskConfig,
    ) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.inner.schedule_task(config)
    }

    fn finish(&self, task_id: &str) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.inner.finish(task_id)
    }

    fn register(&self, request: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.inner.register(request)
    }

    fn is_scheduled(&self, unique_name: &str) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        self.inner.is_scheduled(unique_name)
    }

    fn work_info(
        &self,
        unique_name: &str,
    ) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>> {
        self.inner.work_info(unique_name)
    }

    fn cancel(&self, cancel_by: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>> {
        self.inner.cancel(cancel_by)
    }
}
