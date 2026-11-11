//! A target with no background bridge, the web included: nothing can be scheduled, and every
//! scheduling call fails with `ERR_BACKGROUND_NOT_AVAILABLE`.

use std::future::ready;

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskHandler, WorkRequest},
    error::{BackgroundError, ErrorKind},
    options::{FetchConfig, Status, TaskConfig, WorkInfo},
};

/// The backend of a target background work cannot run on. The platform offers none of the
/// capability, as biometrics found for its prompt, so `status` is restricted, not denied:
/// unavailable with no way to enable it, the references' restricted case.
pub struct UnsupportedBackend;

impl Backend for UnsupportedBackend {
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        Box::pin(ready(Ok::<Status, BackgroundError>(Status::Restricted)))
    }

    fn configure(
        &self,
        _: FetchConfig,
        _: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        Box::pin(ready(Err(BackgroundError::new(ErrorKind::NotAvailable))))
    }

    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        Box::pin(ready(Err(BackgroundError::new(ErrorKind::NotAvailable))))
    }

    fn stop(&self, _: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        Box::pin(ready(Ok::<bool, BackgroundError>(false)))
    }

    fn schedule_task(&self, _: TaskConfig) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        Box::pin(ready(Err(BackgroundError::new(ErrorKind::NotAvailable))))
    }

    fn finish(&self, _: &str) -> BackendFuture<'_, Result<(), BackgroundError>> {
        Box::pin(ready(Ok::<(), BackgroundError>(())))
    }

    fn register(&self, _: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>> {
        Box::pin(ready(Err(BackgroundError::new(ErrorKind::NotAvailable))))
    }

    fn is_scheduled(&self, _: &str) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        Box::pin(ready(Ok::<bool, BackgroundError>(false)))
    }

    fn work_info(&self, _: &str) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>> {
        Box::pin(ready(Ok::<Option<WorkInfo>, BackgroundError>(None)))
    }

    fn cancel(&self, _: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>> {
        Box::pin(ready(Ok::<(), BackgroundError>(())))
    }
}
