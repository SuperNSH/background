//! The in-process engine every bridge of v0.1 delegates to: tokio timers, one event per
//! firing, nothing armed until a call arms it.
//!
//! Dropping the backend aborts every timer it owns; a timer that fired never fires twice.

use std::{
    collections::HashMap,
    future::ready,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use tokio::{runtime::Handle, task::JoinHandle, time::Instant};

use crate::{
    backend::{Backend, BackendFuture, CancelBy, TaskEvent, TaskHandler, WorkKind, WorkRequest},
    error::{BackgroundError, ErrorKind},
    options::{
        ExistingPeriodicWorkPolicy, ExistingWorkPolicy, FetchConfig, Status, TaskConfig, WorkInfo,
        WorkState,
    },
    scheduler::FETCH_TASK_ID,
};

/// The floor a platform puts under the configured fetch interval. The default of
/// [`Duration::ZERO`] is no floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limits {
    /// The shortest fetch interval this platform allows.
    pub min_fetch_interval: Duration,
}

struct WorkRecord {
    info: WorkInfo,
    timer: Option<JoinHandle<()>>,
    // The firing that waited for a handler: held once, delivered by the next configure.
    pending: Option<TaskEvent>,
    generation: u64,
}

#[derive(Default)]
struct State {
    config: Option<FetchConfig>,
    handler: Option<TaskHandler>,
    fetch_timer: Option<JoinHandle<()>>,
    // The fetch arming a firing must still belong to; every arm and disarm moves it on.
    fetch_generation: u64,
    tasks: HashMap<String, TaskTimer>,
    works: HashMap<String, WorkRecord>,
    // Each registration takes the next one; a firing for an older generation is stale.
    next_generation: u64,
}

/// A scheduled task's timer, with the arming a firing must still belong to.
struct TaskTimer {
    timer: JoinHandle<()>,
    generation: u64,
}

struct Inner {
    limits: Limits,
    state: Mutex<State>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The in-process engine: fetch and task timers on the calling thread's runtime, work
/// records in one map. Every v0.1 platform bridge wraps this with its own [`Limits`].
pub struct LocalBackend {
    inner: Arc<Inner>,
}

impl LocalBackend {
    /// The engine with the platform's floor under the fetch interval.
    pub fn new(limits: Limits) -> Self {
        Self {
            inner: Arc::new(Inner {
                limits,
                state: Mutex::new(State::default()),
            }),
        }
    }
}

impl Drop for LocalBackend {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        if let Some(timer) = state.fetch_timer.take() {
            timer.abort();
        }
        for (_, TaskTimer { timer, .. }) in state.tasks.drain() {
            timer.abort();
        }
        for record in state.works.values_mut() {
            if let Some(timer) = record.timer.take() {
                timer.abort();
            }
        }
    }
}

/// Which arming a fetch or task firing must still belong to.
enum FireToken<'a> {
    Fetch(u64),
    Task { id: &'a str, generation: u64 },
}

/// Delivers one fetch or task event to the configured handler, outside the state lock —
/// but only when this firing's arming is still the current one: an abort-immune in-flight
/// firing of a retired timer resolves as a silent no-op.
fn fire(inner: &Inner, token: FireToken<'_>, task_id: &str) {
    let handler = {
        let state = inner.lock();
        let current = match token {
            FireToken::Fetch(generation) => state.fetch_generation == generation,
            FireToken::Task { id, generation } => state
                .tasks
                .get(id)
                .is_some_and(|timer| timer.generation == generation),
        };
        if !current {
            return;
        }
        state.handler.clone()
    };
    if let Some(handler) = handler {
        handler(TaskEvent {
            task_id: task_id.to_owned(),
            timed_out: false,
            input: Vec::new(),
        });
    }
}

/// The next deadline of a fixed-rate period: the chained one after the deadline just
/// fired, skipping ticks already missed — handler latency never stretches the period,
/// and missed ticks never burst.
fn next_deadline(previous: Instant, interval: Duration) -> Instant {
    let now = Instant::now();
    let mut next = previous + interval;
    while next <= now {
        next += interval;
    }
    next
}

async fn fetch_loop(inner: Arc<Inner>, generation: u64, first: Instant, interval: Duration) {
    let mut deadline = first;
    tokio::time::sleep_until(deadline).await;
    loop {
        fire(&inner, FireToken::Fetch(generation), FETCH_TASK_ID);
        deadline = next_deadline(deadline, interval);
        tokio::time::sleep_until(deadline).await;
    }
}

async fn task_once(inner: Arc<Inner>, task_id: String, generation: u64, first: Instant) {
    tokio::time::sleep_until(first).await;
    fire(
        &inner,
        FireToken::Task {
            id: &task_id,
            generation,
        },
        &task_id,
    );
}

async fn task_loop(
    inner: Arc<Inner>,
    task_id: String,
    generation: u64,
    first: Instant,
    period: Duration,
) {
    let mut deadline = first;
    tokio::time::sleep_until(deadline).await;
    loop {
        fire(
            &inner,
            FireToken::Task {
                id: &task_id,
                generation,
            },
            &task_id,
        );
        deadline = next_deadline(deadline, period);
        tokio::time::sleep_until(deadline).await;
    }
}

/// What a work timer's firing came to.
enum Firing {
    /// The event went to the handler.
    Fired,
    /// No handler is configured; the event is held in the record, at most one per work
    /// item, for the next configure to deliver.
    Held,
    /// The record is not this timer's any more: replaced, cancelled, stopped, or already
    /// terminal. The firing delivers nothing and changes nothing.
    Stale,
}

/// Marks the work under `unique_name` running, then delivers its one event or holds it for
/// the next configure — but only the record at this timer's own generation while it is
/// still Scheduled. Anything else is a firing abort could not stop (in-flight past its
/// sleep on a multithreaded runtime) for a registration that has moved on; it must not
/// resurrect the record or deliver its event.
fn run_work(
    inner: &Inner,
    unique_name: &str,
    generation: u64,
    task_name: &str,
    input: &crate::options::Input,
) -> Firing {
    let handler = {
        let mut state = inner.lock();
        let handler = state.handler.clone();
        let Some(record) = state.works.get_mut(unique_name) else {
            return Firing::Stale;
        };
        if record.generation != generation {
            return Firing::Stale;
        }
        match record.info.state {
            WorkState::Scheduled => {}
            // An earlier tick of this timer already held its event; nothing new to hold.
            WorkState::Running => return Firing::Held,
            WorkState::Cancelled | WorkState::Succeeded | WorkState::Failed => {
                return Firing::Stale;
            }
        }
        record.info.state = WorkState::Running;
        match handler {
            Some(handler) => handler,
            None => {
                if record.pending.is_none() {
                    record.pending = Some(TaskEvent {
                        task_id: task_name.to_owned(),
                        timed_out: false,
                        input: input.clone(),
                    });
                }
                return Firing::Held;
            }
        }
    };
    handler(TaskEvent {
        task_id: task_name.to_owned(),
        timed_out: false,
        input: input.clone(),
    });
    Firing::Fired
}

/// Sets the work's state after its event, but only the record this timer just set running
/// at its own generation: one a replacement re-armed is not this timer's to finish.
fn settle_work(inner: &Inner, unique_name: &str, generation: u64, state_after: WorkState) {
    let mut state = inner.lock();
    if let Some(record) = state.works.get_mut(unique_name)
        && record.generation == generation
        && record.info.state == WorkState::Running
    {
        record.info.state = state_after;
        if state_after == WorkState::Succeeded {
            record.timer = None;
        }
    }
}

/// Finishes a held firing its configure just delivered — but only the record that held
/// it, at its own generation: one a replacement re-armed during the delivery window is
/// not this delivery's to settle.
fn settle_held(inner: &Inner, unique_name: &str, generation: u64, is_periodic: bool) {
    let mut state = inner.lock();
    if let Some(record) = state.works.get_mut(unique_name)
        && record.generation == generation
        && record.info.state == WorkState::Running
    {
        record.info.state = if is_periodic {
            WorkState::Scheduled
        } else {
            WorkState::Succeeded
        };
        if !is_periodic {
            record.timer = None;
        }
    }
}

async fn work_once(
    inner: Arc<Inner>,
    unique_name: String,
    generation: u64,
    task_name: String,
    input: crate::options::Input,
    first: Instant,
) {
    tokio::time::sleep_until(first).await;
    match run_work(&inner, &unique_name, generation, &task_name, &input) {
        // Held stays Running until the next configure delivers the event and finishes it.
        Firing::Held | Firing::Stale => {}
        Firing::Fired => settle_work(&inner, &unique_name, generation, WorkState::Succeeded),
    }
}

async fn work_loop(
    inner: Arc<Inner>,
    unique_name: String,
    generation: u64,
    task_name: String,
    input: crate::options::Input,
    first: Instant,
    frequency: Duration,
) {
    let mut deadline = first;
    tokio::time::sleep_until(deadline).await;
    loop {
        match run_work(&inner, &unique_name, generation, &task_name, &input) {
            // Stale: the registration moved on; this timer stops firing.
            Firing::Stale => return,
            // Held keeps its period: the loop sleeps on, later ticks hold nothing new.
            Firing::Held => {}
            Firing::Fired => settle_work(&inner, &unique_name, generation, WorkState::Scheduled),
        }
        deadline = next_deadline(deadline, frequency);
        tokio::time::sleep_until(deadline).await;
    }
}

impl Backend for LocalBackend {
    fn status(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        Box::pin(ready(Ok(Status::Available)))
    }

    fn configure(
        &self,
        config: FetchConfig,
        handler: TaskHandler,
    ) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            // Work that fired before any handler existed holds its event; this configure
            // delivers what it held, outside the lock.
            let held: Vec<(String, TaskEvent, bool, u64)> = {
                let mut state = inner.lock();
                // The platform's floor applies under the configured interval.
                let mut config = config;
                if config.minimum_fetch_interval < inner.limits.min_fetch_interval {
                    config.minimum_fetch_interval = inner.limits.min_fetch_interval;
                }
                state.config = Some(config);
                state.handler = Some(handler.clone());
                state
                    .works
                    .iter_mut()
                    .filter(|(_, record)| record.info.state == WorkState::Running)
                    .filter_map(|(name, record)| {
                        let (is_periodic, generation) =
                            (record.info.is_periodic, record.generation);
                        record
                            .pending
                            .take()
                            .map(|event| (name.clone(), event, is_periodic, generation))
                    })
                    .collect()
            };
            for (name, event, is_periodic, generation) in held {
                handler(event);
                settle_held(&inner, &name, generation, is_periodic);
            }
            Ok(Status::Available)
        })
    }

    fn start(&self) -> BackendFuture<'_, Result<Status, BackgroundError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            // Timers need a tokio driver on this thread; without one, arming is a coded
            // failure rather than a panic.
            let handle = runtime_handle()?;
            let mut state = inner.lock();
            let interval = match state.config {
                Some(config) => config.minimum_fetch_interval,
                None => return Err(BackgroundError::new(ErrorKind::NotConfigured)),
            };
            if let Some(timer) = state.fetch_timer.take() {
                timer.abort();
            }
            let generation = state.next_generation;
            state.next_generation += 1;
            state.fetch_generation = generation;
            // The first wait anchors here, at the arming call: a clock that moves before
            // the task's first poll must not stretch it.
            let first = Instant::now() + interval;
            state.fetch_timer =
                Some(handle.spawn(fetch_loop(inner.clone(), generation, first, interval)));
            Ok(Status::Available)
        })
    }

    fn stop(&self, task_id: Option<&str>) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let inner = self.inner.clone();
        let task_id = task_id.map(str::to_owned);
        Box::pin(async move {
            let mut state = inner.lock();
            let Some(task_id) = task_id else {
                let mut stopped = false;
                if let Some(timer) = state.fetch_timer.take() {
                    timer.abort();
                    // A disarm retires the token too, so an in-flight firing cannot land.
                    state.fetch_generation = state.next_generation;
                    state.next_generation += 1;
                    stopped = true;
                }
                for (_, TaskTimer { timer, .. }) in state.tasks.drain() {
                    timer.abort();
                    stopped = true;
                }
                for record in state.works.values_mut() {
                    // Live work stops like cancel() stops it: timer aborted, record
                    // cancelled, any held event dropped. Terminal records keep their state.
                    if matches!(record.info.state, WorkState::Scheduled | WorkState::Running) {
                        if let Some(timer) = record.timer.take() {
                            timer.abort();
                        }
                        record.pending = None;
                        record.info.state = WorkState::Cancelled;
                        stopped = true;
                    }
                }
                return Ok(stopped);
            };
            match state.tasks.remove(&task_id) {
                Some(TaskTimer { timer, .. }) => {
                    timer.abort();
                    Ok(true)
                }
                None => Ok(false),
            }
        })
    }

    fn schedule_task(
        &self,
        config: TaskConfig,
    ) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let handle = runtime_handle()?;
            let TaskConfig {
                task_id,
                delay,
                periodic,
                ..
            } = config;
            let mut state = inner.lock();
            if let Some(TaskTimer { timer, .. }) = state.tasks.remove(&task_id) {
                timer.abort();
            }
            let generation = state.next_generation;
            state.next_generation += 1;
            // The first wait anchors at this call, not at the task's first poll.
            let first = Instant::now() + delay;
            let timer = if periodic {
                handle.spawn(task_loop(
                    inner.clone(),
                    task_id.clone(),
                    generation,
                    first,
                    delay,
                ))
            } else {
                handle.spawn(task_once(inner.clone(), task_id.clone(), generation, first))
            };
            state.tasks.insert(task_id, TaskTimer { timer, generation });
            Ok(true)
        })
    }

    fn finish(&self, _task_id: &str) -> BackendFuture<'_, Result<(), BackgroundError>> {
        Box::pin(ready(Ok(())))
    }

    fn register(&self, request: WorkRequest) -> BackendFuture<'_, Result<(), BackgroundError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let handle = runtime_handle()?;
            let mut state = inner.lock();
            let WorkRequest {
                kind,
                unique_name,
                task_name,
                input,
                initial_delay,
                frequency,
                flex_interval: _,
                constraints: _,
                existing_work_policy,
                existing_periodic_work_policy,
                backoff_policy: _,
                backoff_policy_delay: _,
                tag,
            } = request;
            // An absent policy keeps the existing work, workmanager's own default.
            let keep = match kind {
                WorkKind::OneOff => {
                    matches!(existing_work_policy, None | Some(ExistingWorkPolicy::Keep))
                }
                WorkKind::Periodic => matches!(
                    existing_periodic_work_policy,
                    None | Some(ExistingPeriodicWorkPolicy::Keep)
                ),
            };
            let live = matches!(
                state.works.get(&unique_name),
                Some(record) if matches!(record.info.state, WorkState::Scheduled | WorkState::Running)
            );
            if keep && live {
                return Ok(());
            }
            if let Some(record) = state.works.remove(&unique_name)
                && let Some(timer) = record.timer
            {
                timer.abort();
            }
            let is_periodic = matches!(kind, WorkKind::Periodic);
            // The new registration's generation: an older timer's in-flight firing on it
            // is stale, however the old timer was retired.
            let generation = state.next_generation;
            state.next_generation += 1;
            let mut record = WorkRecord {
                info: WorkInfo {
                    unique_name: unique_name.clone(),
                    state: WorkState::Scheduled,
                    is_periodic,
                    task_name: Some(task_name.clone()),
                    tags: tag.into_iter().collect(),
                },
                timer: None,
                pending: None,
                generation,
            };
            let delay = initial_delay.unwrap_or(Duration::ZERO);
            // The first wait anchors at this call, not at the task's first poll.
            let first = Instant::now() + delay;
            record.timer = Some(if is_periodic {
                // A period work's unset frequency is workmanager's own 15-minute minimum.
                let frequency = frequency.unwrap_or(Duration::from_secs(15 * 60));
                handle.spawn(work_loop(
                    inner.clone(),
                    unique_name.clone(),
                    generation,
                    task_name,
                    input,
                    first,
                    frequency,
                ))
            } else {
                handle.spawn(work_once(
                    inner.clone(),
                    unique_name.clone(),
                    generation,
                    task_name,
                    input,
                    first,
                ))
            });
            state.works.insert(unique_name, record);
            Ok(())
        })
    }

    fn is_scheduled(&self, unique_name: &str) -> BackendFuture<'_, Result<bool, BackgroundError>> {
        let inner = self.inner.clone();
        let unique_name = unique_name.to_owned();
        Box::pin(async move {
            let state = inner.lock();
            Ok(matches!(
                state.works.get(&unique_name),
                Some(record) if matches!(record.info.state, WorkState::Scheduled | WorkState::Running)
            ))
        })
    }

    fn work_info(
        &self,
        unique_name: &str,
    ) -> BackendFuture<'_, Result<Option<WorkInfo>, BackgroundError>> {
        let inner = self.inner.clone();
        let unique_name = unique_name.to_owned();
        Box::pin(async move {
            let state = inner.lock();
            Ok(state
                .works
                .get(&unique_name)
                .map(|record| record.info.clone()))
        })
    }

    fn cancel(&self, cancel_by: CancelBy) -> BackendFuture<'_, Result<(), BackgroundError>> {
        let inner = self.inner.clone();
        Box::pin(async move {
            let mut state = inner.lock();
            let cancels: Vec<String> = match cancel_by {
                CancelBy::UniqueName(name) => vec![name],
                CancelBy::Tag(tag) => state
                    .works
                    .values()
                    .filter(|record| record.info.tags.contains(&tag))
                    .map(|record| record.info.unique_name.clone())
                    .collect(),
                CancelBy::All => state
                    .works
                    .values()
                    .map(|record| record.info.unique_name.clone())
                    .collect(),
            };
            for name in cancels {
                if let Some(record) = state.works.get_mut(&name) {
                    if let Some(timer) = record.timer.take() {
                        timer.abort();
                    }
                    record.pending = None;
                    record.info.state = WorkState::Cancelled;
                }
            }
            Ok(())
        })
    }
}

/// The runtime timers spawn on. There being none is the one way arming fails.
fn runtime_handle() -> Result<Handle, BackgroundError> {
    Handle::try_current().map_err(|_| BackgroundError::new(ErrorKind::NotAvailable))
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use super::*;
    use crate::options::{Input, InputValue, NetworkType};
    use crate::testing::{recording, settle};

    const MINUTE: Duration = Duration::from_secs(60);

    /// Configures `backend` with the default 15-minute fetch and hands it a recording
    /// handler; the recorder it returns is where the test reads the events from.
    async fn configured(backend: &LocalBackend) -> Arc<Mutex<Vec<TaskEvent>>> {
        let (events, handler) = recording();
        backend
            .configure(FetchConfig::default(), handler)
            .await
            .unwrap();
        events
    }

    fn count(events: &Arc<Mutex<Vec<TaskEvent>>>) -> usize {
        events.lock().unwrap().len()
    }

    fn one_off_work(
        unique_name: &str,
        task_name: &str,
        delay: Duration,
        input: Input,
    ) -> WorkRequest {
        WorkRequest {
            kind: WorkKind::OneOff,
            unique_name: unique_name.to_owned(),
            task_name: task_name.to_owned(),
            input,
            initial_delay: Some(delay),
            frequency: None,
            flex_interval: None,
            constraints: None,
            existing_work_policy: None,
            existing_periodic_work_policy: None,
            backoff_policy: None,
            backoff_policy_delay: None,
            tag: None,
        }
    }

    fn periodic_work(
        unique_name: &str,
        task_name: &str,
        frequency: Option<Duration>,
    ) -> WorkRequest {
        WorkRequest {
            kind: WorkKind::Periodic,
            frequency,
            ..one_off_work(unique_name, task_name, MINUTE, Vec::new())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn status_is_available_even_before_configure() {
        let backend = LocalBackend::new(Limits::default());
        assert_eq!(backend.status().await, Ok(Status::Available));
    }

    #[tokio::test(start_paused = true)]
    async fn start_without_configure_is_refused() {
        let backend = LocalBackend::new(Limits::default());
        assert_eq!(
            backend.start().await.unwrap_err().code(),
            "ERR_BACKGROUND_NOT_CONFIGURED"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn start_fires_one_fetch_event_per_interval() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        assert_eq!(backend.start().await, Ok(Status::Available));
        settle().await;

        // The first event waits a full interval, and a partial one is not enough.
        tokio::time::advance(14 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 0);
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);

        tokio::time::advance(15 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 2);

        let events = events.lock().unwrap();
        for event in events.iter() {
            assert_eq!(event.task_id, FETCH_TASK_ID);
            assert!(!event.timed_out);
            assert!(event.input.is_empty());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_platform_floor_applies_to_the_fetch_interval() {
        let backend = LocalBackend::new(Limits {
            min_fetch_interval: 30 * MINUTE,
        });
        let (events, handler) = recording();
        let config = FetchConfig {
            required_network_type: NetworkType::Unmetered, // 15-minute interval kept
            ..FetchConfig::default()
        };
        backend.configure(config, handler).await.unwrap();
        backend.start().await.unwrap();
        settle().await;

        tokio::time::advance(29 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 0);
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_scheduled_task_fires_once_or_per_period_under_its_own_id() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;

        backend
            .schedule_task(TaskConfig {
                task_id: "once".to_owned(),
                delay: 5 * MINUTE,
                ..TaskConfig::default()
            })
            .await
            .unwrap();
        backend
            .schedule_task(TaskConfig {
                task_id: "hourly".to_owned(),
                delay: 2 * MINUTE,
                periodic: true,
                ..TaskConfig::default()
            })
            .await
            .unwrap();
        settle().await;

        tokio::time::advance(2 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "hourly");

        tokio::time::advance(3 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 3); // "hourly" twice, "once" once
        let ids: Vec<String> = {
            let events = events.lock().unwrap();
            events[1..]
                .iter()
                .map(|event| event.task_id.clone())
                .collect()
        };
        assert_eq!(ids, ["hourly", "once"]);

        // "once" never fires again; "hourly" keeps firing once per period. A paused clock
        // jumps whole periods at once, so each advance yields exactly one firing.
        for _ in 0..15 {
            tokio::time::advance(2 * MINUTE).await;
            settle().await;
        }
        assert_eq!(count(&events), 3 + 15);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_none_cancels_every_timer_and_answers_whether_anything_stopped() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend.start().await.unwrap();
        backend
            .schedule_task(TaskConfig {
                task_id: "once".to_owned(),
                delay: 5 * MINUTE,
                ..TaskConfig::default()
            })
            .await
            .unwrap();
        backend
            .register(one_off_work("work", "sync", MINUTE, Vec::new()))
            .await
            .unwrap();
        settle().await;

        assert_eq!(backend.stop(None).await, Ok(true));
        tokio::time::advance(60 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 0);
        assert_eq!(backend.stop(None).await, Ok(false));
    }

    #[tokio::test(start_paused = true)]
    async fn stop_with_an_id_cancels_only_that_task() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        for task_id in ["a", "b"] {
            backend
                .schedule_task(TaskConfig {
                    task_id: task_id.to_owned(),
                    delay: 5 * MINUTE,
                    ..TaskConfig::default()
                })
                .await
                .unwrap();
        }
        settle().await;

        assert_eq!(backend.stop(Some("a")).await, Ok(true));
        assert_eq!(backend.stop(Some("a")).await, Ok(false));
        assert_eq!(backend.stop(Some("unknown")).await, Ok(false));
        tokio::time::advance(5 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "b");
    }

    #[tokio::test(start_paused = true)]
    async fn register_runs_one_off_work_once_and_succeeds() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(one_off_work(
                "sync-work",
                "sync",
                Duration::from_secs(30),
                vec![("count".to_owned(), InputValue::Int(3))],
            ))
            .await
            .unwrap();
        settle().await;

        // Scheduled until its delay passes.
        let before = backend.work_info("sync-work").await.unwrap().unwrap();
        assert_eq!(before.state, WorkState::Scheduled);
        assert!(!before.is_periodic);
        assert_eq!(before.task_name, Some("sync".to_owned()));
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(true));

        tokio::time::advance(Duration::from_secs(30)).await;
        settle().await;
        assert_eq!(count(&events), 1);
        let event = events.lock().unwrap()[0].clone();
        assert_eq!(event.task_id, "sync");
        assert!(!event.timed_out);
        assert_eq!(event.input, vec![("count".to_owned(), InputValue::Int(3))]);

        let after = backend.work_info("sync-work").await.unwrap().unwrap();
        assert_eq!(after.state, WorkState::Succeeded);
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(false));

        tokio::time::advance(30 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn register_periodic_defaults_the_frequency_to_a_quarter_hour() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(periodic_work("sync-work", "sync", None))
            .await
            .unwrap();
        settle().await;

        tokio::time::advance(MINUTE).await; // the initial delay
        settle().await;
        assert_eq!(count(&events), 1);
        let running = backend.work_info("sync-work").await.unwrap().unwrap();
        assert_eq!(running.state, WorkState::Scheduled); // back to scheduled after its event
        assert!(running.is_periodic);
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(true));

        tokio::time::advance(14 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn keep_leaves_active_work_alone_and_replace_takes_over() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(one_off_work(
                "sync-work",
                "first",
                2 * MINUTE,
                vec![("run".to_owned(), InputValue::Int(1))],
            ))
            .await
            .unwrap();
        let keep = WorkRequest {
            existing_work_policy: Some(ExistingWorkPolicy::Keep),
            input: vec![("run".to_owned(), InputValue::Int(2))],
            ..one_off_work("sync-work", "second", MINUTE, Vec::new())
        };
        backend.register(keep).await.unwrap();
        settle().await;

        tokio::time::advance(2 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "first");

        // After the first succeeded, Keep registers the new one.
        let again = WorkRequest {
            existing_work_policy: Some(ExistingWorkPolicy::Keep),
            task_name: "third".to_owned(),
            ..one_off_work("sync-work", "third", MINUTE, Vec::new())
        };
        backend.register(again).await.unwrap();
        settle().await;
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 2);
        assert_eq!(events.lock().unwrap()[1].task_id, "third");

        // Replace swaps an active record for the new one.
        backend
            .register(one_off_work("other-work", "old", 5 * MINUTE, Vec::new()))
            .await
            .unwrap();
        let replace = WorkRequest {
            existing_work_policy: Some(ExistingWorkPolicy::Replace),
            input: vec![("run".to_owned(), InputValue::Bool(true))],
            ..one_off_work("other-work", "new", MINUTE, Vec::new())
        };
        backend.register(replace).await.unwrap();
        settle().await;
        tokio::time::advance(5 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 3);
        assert_eq!(events.lock().unwrap()[2].task_id, "new");
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_policies_behave_the_same_way() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(periodic_work("sync-work", "first", Some(MINUTE)))
            .await
            .unwrap();
        let keep = WorkRequest {
            existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Keep),
            ..periodic_work("sync-work", "second", Some(2 * MINUTE))
        };
        backend.register(keep).await.unwrap();
        settle().await;
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "first");

        let update = WorkRequest {
            existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Update),
            ..periodic_work("sync-work", "second", Some(MINUTE))
        };
        backend.register(update).await.unwrap();
        settle().await;
        // The replaced record starts its own delay from the new registration; the old
        // timer is gone, so only the new one fires.
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 2);
        assert_eq!(events.lock().unwrap()[1].task_id, "second");
        tokio::time::advance(MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 3);
        assert_eq!(events.lock().unwrap()[2].task_id, "second");
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_marks_records_cancelled_and_stops_their_timers() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        let mut tagged = one_off_work("tagged", "sync", 5 * MINUTE, Vec::new());
        tagged.tag = Some("uploads".to_owned());
        backend
            .register(one_off_work("named", "sync", 5 * MINUTE, Vec::new()))
            .await
            .unwrap();
        backend.register(tagged).await.unwrap();

        backend
            .cancel(CancelBy::UniqueName("named".to_owned()))
            .await
            .unwrap();
        assert_eq!(
            backend.work_info("named").await.unwrap().unwrap().state,
            WorkState::Cancelled
        );

        backend
            .cancel(CancelBy::Tag("uploads".to_owned()))
            .await
            .unwrap();
        assert_eq!(
            backend.work_info("tagged").await.unwrap().unwrap().state,
            WorkState::Cancelled
        );
        assert_eq!(backend.is_scheduled("tagged").await, Ok(false));

        backend
            .register(one_off_work("third", "sync", 5 * MINUTE, Vec::new()))
            .await
            .unwrap();
        backend.cancel(CancelBy::All).await.unwrap();
        assert_eq!(
            backend.work_info("third").await.unwrap().unwrap().state,
            WorkState::Cancelled
        );

        // A cancelled record answers queries and never fires.
        assert_eq!(backend.work_info("missing").await.unwrap(), None);
        tokio::time::advance(30 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_none_marks_live_work_cancelled_and_leaves_done_work_alone() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(one_off_work("pending", "sync", 5 * MINUTE, Vec::new()))
            .await
            .unwrap();
        backend
            .register(one_off_work("done", "sync", Duration::ZERO, Vec::new()))
            .await
            .unwrap();
        settle().await; // "done" runs at once

        assert_eq!(backend.stop(None).await, Ok(true));
        assert_eq!(
            backend.work_info("pending").await.unwrap().unwrap().state,
            WorkState::Cancelled
        );
        assert_eq!(backend.is_scheduled("pending").await, Ok(false));
        assert_eq!(
            backend.work_info("done").await.unwrap().unwrap().state,
            WorkState::Succeeded
        );

        tokio::time::advance(30 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1); // only "done" ever fired
        assert_eq!(backend.stop(None).await, Ok(false));
    }

    #[tokio::test(start_paused = true)]
    async fn work_registered_before_configure_holds_its_event_for_the_handler() {
        let backend = LocalBackend::new(Limits::default());
        backend
            .register(one_off_work(
                "sync-work",
                "sync",
                MINUTE,
                vec![("count".to_owned(), InputValue::Int(3))],
            ))
            .await
            .unwrap();
        settle().await;
        tokio::time::advance(MINUTE).await;
        settle().await;

        // No handler yet: the event is held, not dropped, and the work is mid-flight.
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Running
        );
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(true));

        let (events, handler) = recording();
        backend
            .configure(FetchConfig::default(), handler)
            .await
            .unwrap();
        settle().await;
        assert_eq!(count(&events), 1);
        let event = events.lock().unwrap()[0].clone();
        assert_eq!(event.task_id, "sync");
        assert!(!event.timed_out);
        assert_eq!(event.input, vec![("count".to_owned(), InputValue::Int(3))]);
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Succeeded
        );
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(false));
    }

    #[tokio::test(start_paused = true)]
    async fn periodic_work_registered_before_configure_keeps_its_period() {
        let backend = LocalBackend::new(Limits::default());
        backend
            .register(periodic_work("sync-work", "sync", Some(2 * MINUTE)))
            .await
            .unwrap();
        settle().await;
        tokio::time::advance(MINUTE).await; // the initial delay
        settle().await;
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Running // its event is held
        );

        let (events, handler) = recording();
        backend
            .configure(FetchConfig::default(), handler)
            .await
            .unwrap();
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Scheduled
        );

        // The timer kept its period; the next firing goes straight to the handler.
        tokio::time::advance(2 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 2);
        assert_eq!(events.lock().unwrap()[1].task_id, "sync");
    }

    #[tokio::test(start_paused = true)]
    async fn update_and_periodic_replace_both_replace_the_registration() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .register(one_off_work("one", "old", 5 * MINUTE, Vec::new()))
            .await
            .unwrap();
        let updated = WorkRequest {
            existing_work_policy: Some(ExistingWorkPolicy::Update),
            ..one_off_work("one", "new", MINUTE, Vec::new())
        };
        backend.register(updated).await.unwrap();
        settle().await;
        tokio::time::advance(5 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "new");

        backend
            .register(periodic_work("loop", "old", Some(MINUTE)))
            .await
            .unwrap();
        let replaced = WorkRequest {
            existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Replace),
            ..periodic_work("loop", "new", Some(5 * MINUTE))
        };
        backend.register(replaced).await.unwrap();
        settle().await;
        tokio::time::advance(5 * MINUTE).await;
        settle().await;
        let ids: Vec<String> = events.lock().unwrap()[1..]
            .iter()
            .map(|event| event.task_id.clone())
            .collect();
        assert_eq!(ids, ["new"]); // the old period never fired
    }

    #[tokio::test(start_paused = true)]
    async fn finish_always_succeeds() {
        let backend = LocalBackend::new(Limits::default());
        backend.finish("known").await.unwrap();
        backend.finish("known").await.unwrap();
        backend.finish("").await.unwrap();
    }

    // A timer's deadline anchors at the arming call, not at its task's first poll: the
    // clock may jump before a busy runtime polls a freshly armed task.
    #[tokio::test(start_paused = true)]
    async fn the_first_fetch_wait_is_anchored_to_start() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend.start().await.unwrap();
        tokio::time::advance(15 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_scheduled_task_first_wait_is_anchored_to_its_scheduling() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend
            .schedule_task(TaskConfig {
                task_id: "once".to_owned(),
                delay: 2 * MINUTE,
                ..TaskConfig::default()
            })
            .await
            .unwrap();
        tokio::time::advance(2 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "once");
    }

    #[tokio::test(start_paused = true)]
    async fn a_work_first_firing_is_anchored_to_its_registration() {
        let backend = LocalBackend::new(Limits::default());
        let (events, handler) = recording();
        backend
            .configure(FetchConfig::default(), handler)
            .await
            .unwrap();
        let work = crate::Workmanager::with_backend(backend);
        work.register_one_off_task(
            "sync-work",
            "sync",
            crate::OneOffOptions {
                initial_delay: Some(2 * MINUTE),
                ..crate::OneOffOptions::default()
            },
        )
        .unwrap();
        tokio::time::advance(2 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
        assert_eq!(events.lock().unwrap()[0].task_id, "sync");
    }

    // The race these two tests guard is unreachable on a paused current-thread runtime:
    // abort is synchronous there. They drive the firing path an abort-immune in-flight
    // timer would take on a multithreaded runtime, straight at run_work and settle_work.
    #[tokio::test(start_paused = true)]
    async fn an_in_flight_firing_of_a_replaced_work_is_a_stale_no_op() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        let old_input: Input = vec![("run".to_owned(), InputValue::Int(1))];
        let fresh_input: Input = vec![("run".to_owned(), InputValue::Int(2))];
        backend
            .register(one_off_work(
                "sync-work",
                "old",
                5 * MINUTE,
                old_input.clone(),
            ))
            .await
            .unwrap();
        settle().await;
        let replaced = WorkRequest {
            existing_work_policy: Some(ExistingWorkPolicy::Replace),
            ..one_off_work("sync-work", "fresh", 10 * MINUTE, fresh_input.clone())
        };
        backend.register(replaced).await.unwrap();
        settle().await;

        // The old generation's firing, driven directly: it must not fire the old payload
        // into the replacement's record or touch its state.
        let firing = run_work(&backend.inner, "sync-work", 0, "old", &old_input);
        assert!(matches!(firing, Firing::Stale));
        assert_eq!(count(&events), 0);
        let info = backend.work_info("sync-work").await.unwrap().unwrap();
        assert_eq!(info.state, WorkState::Scheduled);
        assert_eq!(info.task_name, Some("fresh".to_owned()));

        // The replacement's firing is live; the old generation's settle must not finish
        // it, and its own settle does.
        let firing = run_work(&backend.inner, "sync-work", 1, "fresh", &fresh_input);
        assert!(matches!(firing, Firing::Fired));
        assert_eq!(count(&events), 1);
        settle_work(&backend.inner, "sync-work", 0, WorkState::Succeeded);
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Running
        );
        settle_work(&backend.inner, "sync-work", 1, WorkState::Succeeded);
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Succeeded
        );

        // Disarmed, the work never fires again.
        backend
            .cancel(CancelBy::UniqueName("sync-work".to_owned()))
            .await
            .unwrap();
        tokio::time::advance(30 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn an_in_flight_firing_of_a_cancelled_work_is_a_stale_no_op() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        let no_input: Input = Vec::new();
        backend
            .register(one_off_work("sync-work", "sync", MINUTE, Vec::new()))
            .await
            .unwrap();
        settle().await;
        backend
            .cancel(CancelBy::UniqueName("sync-work".to_owned()))
            .await
            .unwrap();

        // The cancelled record's in-flight firing must not resurrect it.
        let firing = run_work(&backend.inner, "sync-work", 0, "sync", &no_input);
        assert!(matches!(firing, Firing::Stale));
        assert_eq!(count(&events), 0);
        assert_eq!(
            backend.work_info("sync-work").await.unwrap().unwrap().state,
            WorkState::Cancelled
        );
        assert_eq!(backend.is_scheduled("sync-work").await, Ok(false));
    }

    // Driven directly, as the delivery window's replacement race above is: configure hands
    // the handler the held event before it re-locks to settle the record.
    #[tokio::test(start_paused = true)]
    async fn a_held_delivery_settles_only_the_record_that_held_it() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        let no_input: Input = Vec::new();
        backend
            .register(one_off_work("w", "old", 30 * MINUTE, Vec::new()))
            .await
            .unwrap();
        settle().await;
        // Mid-delivery: the old record is running and unsettled.
        assert!(matches!(
            run_work(&backend.inner, "w", 0, "old", &no_input),
            Firing::Fired
        ));
        // The replacement is a periodic, already running its own firing; its policy is
        // the periodic one.
        let replacement = WorkRequest {
            existing_periodic_work_policy: Some(ExistingPeriodicWorkPolicy::Replace),
            ..periodic_work("w", "new", Some(MINUTE))
        };
        backend.register(replacement).await.unwrap();
        settle().await;
        assert!(matches!(
            run_work(&backend.inner, "w", 1, "new", &no_input),
            Firing::Fired
        ));

        // The old delivery's one-off verdict must not settle the replacement's record.
        settle_held(&backend.inner, "w", 0, false);
        assert_eq!(
            backend.work_info("w").await.unwrap().unwrap().state,
            WorkState::Running
        );
        // Its own periodic verdict does.
        settle_held(&backend.inner, "w", 1, true);
        assert_eq!(
            backend.work_info("w").await.unwrap().unwrap().state,
            WorkState::Scheduled
        );
        assert_eq!(count(&events), 2); // both direct firings delivered
    }

    // The superseded-timer races these guard are unreachable on a paused current-thread
    // runtime (abort is synchronous); they drive the abort-immune firing path directly.
    #[tokio::test(start_paused = true)]
    async fn a_superseded_task_firing_delivers_nothing() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        let config = || TaskConfig {
            task_id: "t".to_owned(),
            delay: 5 * MINUTE,
            ..TaskConfig::default()
        };
        backend.schedule_task(config()).await.unwrap();
        settle().await;
        backend.schedule_task(config()).await.unwrap(); // re-scheduling retires the first timer
        settle().await;

        fire(
            &backend.inner,
            FireToken::Task {
                id: "t",
                generation: 0,
            },
            "t",
        ); // the retired timer's in-flight firing
        assert_eq!(count(&events), 0);

        // A stopped id is just as retired: its entry is gone.
        backend.stop(Some("t")).await.unwrap();
        fire(
            &backend.inner,
            FireToken::Task {
                id: "t",
                generation: 1,
            },
            "t",
        );
        assert_eq!(count(&events), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_superseded_fetch_firing_delivers_nothing() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend.start().await.unwrap();
        settle().await;
        backend.start().await.unwrap(); // a second start retires the first fetch timer
        settle().await;

        fire(&backend.inner, FireToken::Fetch(0), FETCH_TASK_ID);
        assert_eq!(count(&events), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn missed_period_ticks_are_skipped_not_shifted() {
        // A handler that ran past two chained deadlines: the next deadline is the first
        // chained one still ahead, not now plus the interval.
        tokio::time::advance(Duration::from_secs(150)).await; // now = 2.5 minutes in
        let fired = Instant::now() - Duration::from_secs(90); // the deadline just fired
        assert_eq!(
            next_deadline(fired, MINUTE),
            Instant::now() + Duration::from_secs(30) // minute 3, not minute 3.5
        );
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_the_backend_aborts_every_timer() {
        let backend = LocalBackend::new(Limits::default());
        let events = configured(&backend).await;
        backend.start().await.unwrap();
        backend
            .schedule_task(TaskConfig {
                task_id: "once".to_owned(),
                delay: MINUTE,
                ..TaskConfig::default()
            })
            .await
            .unwrap();
        backend
            .register(one_off_work("work", "sync", MINUTE, Vec::new()))
            .await
            .unwrap();
        settle().await;

        drop(backend);
        tokio::time::advance(60 * MINUTE).await;
        settle().await;
        assert_eq!(count(&events), 0);
    }
}
