//! A backend with scripted answers. Tests use it, and so can an app that runs without a
//! scheduler.

use std::{
    future::ready,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskEvent, TaskHandler, WorkRequest},
    error::BackgroundError,
    options::{FetchConfig, Status, TaskConfig, WorkInfo},
};

/// A backend whose answers a test sets, whose events a test emits, and whose calls a test
/// counts. Clones share one state.
#[derive(Clone)]
pub struct FakeBackend {
    state: Arc<Mutex<FakeState>>,
}

struct FakeState {
    status: Status,
    configure_error: Option<BackgroundError>,
    start_error: Option<BackgroundError>,
    stop_answer: bool,
    schedule_answer: bool,
    register_error: Option<BackgroundError>,
    is_scheduled: bool,
    work_info: Option<WorkInfo>,
    cancel_error: Option<BackgroundError>,
    handler: Option<TaskHandler>,
    last_config: Option<FetchConfig>,
    last_request: Option<WorkRequest>,
    configures: usize,
    starts: usize,
    stops: usize,
    schedules: usize,
    finishes: usize,
    registers: usize,
    cancels: usize,
}

impl Default for FakeBackend {
    /// A backend that answers available and refuses nothing.
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// A backend that answers available and refuses nothing.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                status: Status::Available,
                configure_error: None,
                start_error: None,
                stop_answer: false,
                schedule_answer: false,
                register_error: None,
                is_scheduled: false,
                work_info: None,
                cancel_error: None,
                handler: None,
                last_config: None,
                last_request: None,
                configures: 0,
                starts: 0,
                stops: 0,
                schedules: 0,
                finishes: 0,
                registers: 0,
                cancels: 0,
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets what `status` answers.
    pub fn set_status(&self, status: Status) {
        self.lock().status = status;
    }

    /// Makes `configure` fail with this error, or clears the failure with `None`.
    pub fn set_configure_error(&self, error: Option<BackgroundError>) {
        self.lock().configure_error = error;
    }

    /// Makes `start` fail with this error, or clears the failure with `None`.
    pub fn set_start_error(&self, error: Option<BackgroundError>) {
        self.lock().start_error = error;
    }

    /// Sets what `stop` answers.
    pub fn set_stop_answer(&self, answer: bool) {
        self.lock().stop_answer = answer;
    }

    /// Sets what `schedule_task` answers.
    pub fn set_schedule_answer(&self, answer: bool) {
        self.lock().schedule_answer = answer;
    }

    /// Makes `register` fail with this error, or clears the failure with `None`.
    pub fn set_register_error(&self, error: Option<BackgroundError>) {
        self.lock().register_error = error;
    }

    /// Sets what `is_scheduled` answers.
    pub fn set_is_scheduled(&self, answer: bool) {
        self.lock().is_scheduled = answer;
    }

    /// Sets what `work_info` answers.
    pub fn set_work_info(&self, info: Option<WorkInfo>) {
        self.lock().work_info = info;
    }

    /// Makes `cancel` fail with this error, or clears the failure with `None`.
    pub fn set_cancel_error(&self, error: Option<BackgroundError>) {
        self.lock().cancel_error = error;
    }

    /// How many times `configure` ran.
    pub fn configures(&self) -> usize {
        self.lock().configures
    }

    /// How many times `start` ran.
    pub fn starts(&self) -> usize {
        self.lock().starts
    }

    /// How many times `stop` ran.
    pub fn stops(&self) -> usize {
        self.lock().stops
    }

    /// How many times `schedule_task` ran.
    pub fn schedules(&self) -> usize {
        self.lock().schedules
    }

    /// How many times `finish` ran.
    pub fn finishes(&self) -> usize {
        self.lock().finishes
    }

    /// How many times `register` ran.
    pub fn registers(&self) -> usize {
        self.lock().registers
    }

    /// How many times `cancel` ran.
    pub fn cancels(&self) -> usize {
        self.lock().cancels
    }

    /// Emits an event to the configured handler, as a platform timer would.
    pub fn emit(&self, task_id: impl Into<String>, timed_out: bool, input: crate::options::Input) {
        let handler = self.lock().handler.clone();
        if let Some(handler) = handler {
            handler(TaskEvent {
                task_id: task_id.into(),
                timed_out,
                input,
            });
        }
    }

    /// Whether a handler is configured.
    pub fn has_handler(&self) -> bool {
        self.lock().handler.is_some()
    }

    /// The config the last `configure` stored.
    pub fn last_config(&self) -> Option<FetchConfig> {
        self.lock().last_config
    }

    /// The work request the last `register` stored.
    pub fn last_request(&self) -> Option<WorkRequest> {
        self.lock().last_request.clone()
    }
}

impl Backend for FakeBackend {
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        let status = self.lock().status;
        Box::pin(ready(Ok(status)))
    }

    fn configure(
        &self,
        config: FetchConfig,
        handler: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        let mut state = self.lock();
        let answer = match state.configure_error.take() {
            Some(error) => Err(error),
            None => Ok(state.status),
        };
        state.configures += 1;
        state.last_config = Some(config);
        state.handler = Some(handler);
        Box::pin(ready(answer))
    }

    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        let mut state = self.lock();
        let answer = match state.start_error.take() {
            Some(error) => Err(error),
            None => Ok(state.status),
        };
        state.starts += 1;
        Box::pin(ready(answer))
    }

    fn stop(&self, _task_id: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let answer = {
            let mut state = self.lock();
            state.stops += 1;
            state.stop_answer
        };
        Box::pin(ready(Ok(answer)))
    }

    fn schedule_task(
        &self,
        _config: TaskConfig,
    ) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let answer = {
            let mut state = self.lock();
            state.schedules += 1;
            state.schedule_answer
        };
        Box::pin(ready(Ok(answer)))
    }

    fn finish(&self, _task_id: &str) -> BackendFuture<'_, Result<(), BackgroundError>> {
        {
            let mut state = self.lock();
            state.finishes += 1;
        }
        Box::pin(ready(Ok(())))
    }

    fn register(&self, request: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>> {
        let mut state = self.lock();
        let answer = match state.register_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        };
        state.registers += 1;
        state.last_request = Some(request);
        Box::pin(ready(answer))
    }

    fn is_scheduled(&self, _unique_name: &str) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let answer = { self.lock().is_scheduled };
        Box::pin(ready(Ok(answer)))
    }

    fn work_info(
        &self,
        _unique_name: &str,
    ) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>> {
        let answer = { self.lock().work_info.clone() };
        Box::pin(ready(Ok(answer)))
    }

    fn cancel(&self, _cancel_by: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>> {
        let mut state = self.lock();
        let answer = match state.cancel_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        };
        state.cancels += 1;
        Box::pin(ready(answer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{backend::resolve_now, options::InputValue};

    #[test]
    fn configure_stores_and_counts_and_emit_fires_the_handler() {
        let backend = FakeBackend::new();
        assert!(!backend.has_handler());

        let (events, handler) = crate::testing::recording();
        let status = resolve_now(backend.configure(FetchConfig::default(), handler)).unwrap();
        assert_eq!(status, Status::Available);
        assert_eq!(backend.configures(), 1);
        assert!(backend.has_handler());
        assert_eq!(
            backend.last_config().unwrap().minimum_fetch_interval,
            FetchConfig::default().minimum_fetch_interval
        );

        backend.emit(
            "sync",
            false,
            vec![("count".to_owned(), InputValue::Int(1))],
        );
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].task_id, "sync");
        assert!(!events[0].timed_out);
        assert_eq!(events[0].input.len(), 1);
    }
}
