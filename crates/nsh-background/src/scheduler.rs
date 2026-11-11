//! `BackgroundFetch`: the periodic-fetch scheduler, with the validation the references apply
//! before any backend sees a call.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use crate::{
    backend::{Backend, TaskEvent, TaskHandler, resolve_now},
    error::{BackgroundError, ErrorKind},
    options::{FetchConfig, Status, TaskConfig},
};

/// The task id of the periodic fetch event, the references' own value.
pub const FETCH_TASK_ID: &str = "flutter_background_fetch";

struct Shared {
    backend: Arc<dyn Backend>,
    // Configure-before-start is this type's rule; the backend only stores state.
    configured: Mutex<bool>,
}

impl Shared {
    fn configured(&self) -> MutexGuard<'_, bool> {
        self.configured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// The periodic-fetch scheduler, as `background_fetch` names it.
///
/// Clones share one scheduler. [`BackgroundFetch::new`] returns the platform's shared handle,
/// so every handle in one app shares it too.
#[derive(Clone)]
pub struct BackgroundFetch {
    shared: Arc<Shared>,
}

impl BackgroundFetch {
    /// The platform's scheduler over the process's one backend.
    pub fn new() -> Self {
        static PLATFORM: OnceLock<BackgroundFetch> = OnceLock::new();
        PLATFORM
            .get_or_init(|| Self::from_arc(crate::platform::shared()))
            .clone()
    }

    /// A scheduler through `backend`, with its own state. Tests and fakes use this.
    pub fn with_backend(backend: impl Backend + 'static) -> Self {
        Self::from_arc(Arc::new(backend))
    }

    fn from_arc(backend: Arc<dyn Backend>) -> Self {
        Self {
            shared: Arc::new(Shared {
                backend,
                configured: Mutex::new(false),
            }),
        }
    }

    /// Stores the fetch settings and the handler every event goes to. A zero
    /// `minimum_fetch_interval` is refused; the platform's floor applies after it.
    pub fn configure(
        &self,
        config: FetchConfig,
        handler: impl Fn(TaskEvent) + Send + Sync + 'static,
    ) -> Result<Status, BackgroundError> {
        if config.minimum_fetch_interval.is_zero() {
            return Err(BackgroundError::new(ErrorKind::InvalidArgument));
        }
        let handler: TaskHandler = Arc::new(handler);
        let status = resolve_now(self.shared.backend.configure(config, handler))?;
        *self.shared.configured() = true;
        Ok(status)
    }

    /// Arms the periodic fetch timer. Requires [`BackgroundFetch::configure`] first.
    pub fn start(&self) -> Result<Status, BackgroundError> {
        if !*self.shared.configured() {
            return Err(BackgroundError::new(ErrorKind::NotConfigured));
        }
        resolve_now(self.shared.backend.start())
    }

    /// Stops the fetch and every timer, or just the scheduled task `task_id` names. An empty
    /// id is refused.
    pub fn stop(&self, task_id: Option<&str>) -> Result<bool, BackgroundError> {
        if task_id.is_some_and(|id| id.trim().is_empty()) {
            return Err(BackgroundError::new(ErrorKind::InvalidArgument));
        }
        resolve_now(self.shared.backend.stop(task_id))
    }

    /// Whether background work runs here.
    pub fn status(&self) -> Result<Status, BackgroundError> {
        resolve_now(self.shared.backend.status())
    }

    /// Arms a task's own timer, once or on a period. Requires configure first; an empty or
    /// whitespace `task_id` is refused.
    pub fn schedule_task(&self, config: TaskConfig) -> Result<bool, BackgroundError> {
        if !*self.shared.configured() {
            return Err(BackgroundError::new(ErrorKind::NotConfigured));
        }
        if config.task_id.trim().is_empty() {
            return Err(BackgroundError::new(ErrorKind::InvalidArgument));
        }
        resolve_now(self.shared.backend.schedule_task(config))
    }

    /// Tells the engine the handler is done with a task. Always succeeds; an unknown id
    /// is a no-op.
    pub fn finish(&self, task_id: &str) -> Result<(), BackgroundError> {
        resolve_now(self.shared.backend.finish(task_id))
    }
}

impl Default for BackgroundFetch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::fake::FakeBackend;
    use crate::options::NetworkType;

    fn scheduler() -> (BackgroundFetch, FakeBackend) {
        let backend = FakeBackend::new();
        let fetch = BackgroundFetch::with_backend(backend.clone());
        (fetch, backend)
    }

    #[test]
    fn configure_refuses_a_zero_interval_without_touching_the_backend() {
        let (fetch, backend) = scheduler();
        let config = FetchConfig {
            minimum_fetch_interval: Duration::ZERO,
            ..FetchConfig::default()
        };
        let error = fetch.configure(config, |_| {}).unwrap_err();
        assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");
        assert_eq!(backend.configures(), 0);
    }

    #[test]
    fn start_and_schedule_task_require_configure_first() {
        let (fetch, backend) = scheduler();
        assert_eq!(
            fetch.start().unwrap_err().code(),
            "ERR_BACKGROUND_NOT_CONFIGURED"
        );
        assert_eq!(
            fetch
                .schedule_task(TaskConfig {
                    task_id: "sync".to_owned(),
                    delay: Duration::from_secs(60),
                    ..TaskConfig::default()
                })
                .unwrap_err()
                .code(),
            "ERR_BACKGROUND_NOT_CONFIGURED"
        );
        assert_eq!(backend.starts(), 0);
        assert_eq!(backend.schedules(), 0);

        fetch.configure(FetchConfig::default(), |_| {}).unwrap();
        fetch.start().unwrap();
        assert_eq!(backend.starts(), 1);
    }

    #[test]
    fn schedule_task_refuses_a_blank_task_id() {
        let (fetch, backend) = scheduler();
        fetch.configure(FetchConfig::default(), |_| {}).unwrap();
        for task_id in ["", "  "] {
            let error = fetch
                .schedule_task(TaskConfig {
                    task_id: task_id.to_owned(),
                    ..TaskConfig::default()
                })
                .unwrap_err();
            assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");
        }
        assert_eq!(backend.schedules(), 0);

        fetch
            .schedule_task(TaskConfig {
                task_id: "sync".to_owned(),
                delay: Duration::from_secs(60),
                periodic: true,
                ..TaskConfig::default()
            })
            .unwrap();
        assert_eq!(backend.schedules(), 1);
    }

    #[test]
    fn stop_refuses_an_empty_task_id() {
        let (fetch, backend) = scheduler();
        assert_eq!(
            fetch.stop(Some("")).unwrap_err().code(),
            "ERR_BACKGROUND_INVALID_ARGUMENT"
        );
        assert_eq!(backend.stops(), 0);
    }

    #[test]
    fn status_stop_and_finish_answer_without_configure() {
        let (fetch, backend) = scheduler();
        backend.set_status(crate::options::Status::Denied);
        assert_eq!(fetch.status().unwrap(), crate::options::Status::Denied);
        assert!(!fetch.stop(None).unwrap());
        fetch.finish("any").unwrap();
        assert_eq!(backend.stops(), 1);
        assert_eq!(backend.finishes(), 1);
    }

    #[test]
    fn clones_share_one_scheduler() {
        let (fetch, backend) = scheduler();
        fetch
            .clone()
            .configure(FetchConfig::default(), |_| {})
            .unwrap();
        fetch.start().unwrap();
        assert_eq!(backend.starts(), 1);
    }

    #[test]
    fn configure_stores_the_config_and_surfaces_a_backend_error() {
        let (fetch, backend) = scheduler();
        let config = FetchConfig {
            minimum_fetch_interval: Duration::from_secs(30 * 60),
            required_network_type: NetworkType::Unmetered,
            ..FetchConfig::default()
        };
        fetch.configure(config, |_| {}).unwrap();
        assert_eq!(backend.configures(), 1);
        let stored = backend.last_config().unwrap();
        assert_eq!(stored.minimum_fetch_interval, Duration::from_secs(30 * 60));
        assert_eq!(stored.required_network_type, NetworkType::Unmetered);

        let refused = FakeBackend::new();
        refused.set_configure_error(Some(BackgroundError::new(ErrorKind::Backend)));
        let fetch = BackgroundFetch::with_backend(refused.clone());
        assert_eq!(
            fetch
                .configure(FetchConfig::default(), |_| {})
                .unwrap_err()
                .code(),
            "ERR_BACKGROUND_BACKEND"
        );
        // A failed configure leaves the scheduler unconfigured.
        assert_eq!(
            fetch.start().unwrap_err().code(),
            "ERR_BACKGROUND_NOT_CONFIGURED"
        );
    }
}
