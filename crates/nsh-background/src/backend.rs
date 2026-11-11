//! The seam between the app-facing types and a platform: the calls, the events, and the work
//! requests.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use crate::{
    error::BackgroundError,
    options::{
        BackoffPolicy, Constraints, ExistingPeriodicWorkPolicy, ExistingWorkPolicy, FetchConfig,
        Input, Status, TaskConfig, WorkInfo,
    },
};

/// A future a backend returns. Each call's future borrows the backend.
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// One event the handler receives: the fetch's own event, a scheduled task's, or a piece of
/// work's.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskEvent {
    /// The id the event carries: the fetch task id, a scheduled task's own id, or a piece of
    /// work's task name.
    pub task_id: String,
    /// Whether the platform gave up on the task's window. In-process timers never time out.
    pub timed_out: bool,
    /// The work's input data. Fetch and scheduled-task events carry none.
    pub input: Input,
}

/// Receives every event the backend fires.
pub type TaskHandler = Arc<dyn Fn(TaskEvent) + Send + Sync>;

/// What a cancel call targets: one unique name, one tag, or everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelBy {
    /// The work registered under this unique name.
    UniqueName(String),
    /// Every work registered with this tag.
    Tag(String),
    /// Every piece of work.
    All,
}

/// Whether a piece of work fires once or on a period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkKind {
    /// Fires once.
    OneOff,
    /// Fires on every period.
    Periodic,
}

/// A piece of work to register, as [`crate::Workmanager`] builds it.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkRequest {
    /// Whether the work fires once or on a period.
    pub kind: WorkKind,
    /// The name the work is known by.
    pub unique_name: String,
    /// The task the work's event names.
    pub task_name: String,
    /// The work's input data.
    pub input: Input,
    /// How long to wait before the first firing. `None` runs at once.
    pub initial_delay: Option<std::time::Duration>,
    /// The period of periodic work. `None` is the 15-minute default.
    pub frequency: Option<std::time::Duration>,
    /// How far into a period the work may still fire.
    pub flex_interval: Option<std::time::Duration>,
    /// The conditions the work waits for.
    pub constraints: Option<Constraints>,
    /// What a registration under a taken name does to the existing one-off work.
    pub existing_work_policy: Option<ExistingWorkPolicy>,
    /// What a registration under a taken name does to the existing periodic work.
    pub existing_periodic_work_policy: Option<ExistingPeriodicWorkPolicy>,
    /// How a failed attempt waits before its next one.
    pub backoff_policy: Option<BackoffPolicy>,
    /// The first backoff delay.
    pub backoff_policy_delay: Option<std::time::Duration>,
    /// A name to cancel the work by, besides its unique name.
    pub tag: Option<String>,
}

/// What a platform provides. [`crate::BackgroundFetch`] and [`crate::Workmanager`] add the
/// rules: validation lives there, not in a backend.
///
/// Every future a backend of this crate returns resolves on its first poll; nothing waits on
/// I/O. [`resolve_now`] relies on that.
pub trait Backend: Send + Sync {
    /// Whether background work runs here.
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>>;

    /// Stores the fetch settings and the handler every event goes to.
    fn configure(
        &self,
        config: FetchConfig,
        handler: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>>;

    /// Arms the periodic fetch timer.
    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>>;

    /// `None` stops the fetch and every timer; an id stops that one scheduled task. Whether
    /// anything stopped.
    fn stop(&self, task_id: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>>;

    /// Arms a task's own timer, once or on a period.
    fn schedule_task(&self, config: TaskConfig)
    -> BackendFuture<'_, Result<bool, BackgroundError>>;

    /// Tells the engine the handler is done with a task. Always succeeds; an unknown id
    /// is a no-op.
    fn finish(&self, task_id: &str) -> BackendFuture<'_, Result<(), BackgroundError>>;

    /// Stores a piece of work, to run after its delay.
    fn register(&self, request: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>>;

    /// Whether the work under this unique name is scheduled or running.
    fn is_scheduled(&self, unique_name: &str) -> BackendFuture<'_, Result<bool, BackgroundError>>;

    /// The work under this unique name, when it exists.
    fn work_info(
        &self,
        unique_name: &str,
    ) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>>;

    /// Cancels work by unique name, by tag, or all of it.
    fn cancel(&self, cancel_by: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>>;
}

/// Polls a backend call to its end without a reactor. Every backend this crate ships resolves
/// on the first poll; one that does not is a bug, and this panics rather than hang.
pub(crate) fn resolve_now<T>(future: BackendFuture<'_, T>) -> T {
    let mut future = future;
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a backend future must resolve on its first poll"),
    }
}
