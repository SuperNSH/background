//! The apple bridge: the engine under Apple's fetch floor. iOS never fires a background fetch
//! faster than every 15 minutes, so a shorter configured interval is lifted to one. Until the
//! BGTaskScheduler integration lands (docs/parity.md), execution is in-process.

use std::time::Duration;

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskHandler, WorkRequest},
    error::BackgroundError,
    local::{Limits, LocalBackend},
    options::{FetchConfig, Status, TaskConfig, WorkInfo},
};

/// The apple backend of v0.1: [`LocalBackend`] under Apple's floor for background fetch.
pub(crate) struct NativeBackend(LocalBackend);

impl NativeBackend {
    /// The engine with Apple's 15-minute floor under the fetch interval.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{recording, settle};

    const MINUTE: Duration = Duration::from_secs(60);

    #[tokio::test(start_paused = true)]
    async fn the_apple_floor_lifts_a_one_minute_fetch_config_to_fifteen_minutes() {
        let backend = NativeBackend::new();
        let (events, handler) = recording();
        backend
            .configure(
                FetchConfig {
                    minimum_fetch_interval: MINUTE,
                    ..FetchConfig::default()
                },
                handler,
            )
            .await
            .unwrap();
        backend.start().await.unwrap();
        settle().await;

        // A smaller floor would already have fired; the first fetch waits the full quarter hour.
        tokio::time::advance(14 * MINUTE).await;
        settle().await;
        assert_eq!(events.lock().unwrap().len(), 0);
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(events.lock().unwrap().len(), 1);
    }
}
