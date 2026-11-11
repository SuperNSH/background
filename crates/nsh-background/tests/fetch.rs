//! The pinned fetch tests: what fires, when, and what each control call answers.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use nsh_background::{
    BackgroundFetch, FETCH_TASK_ID, FakeBackend, FetchConfig, Limits, LocalBackend, Status,
    TaskConfig, TaskEvent,
};

const MINUTE: Duration = Duration::from_secs(60);

/// The recorded events and a handler that records into them; one per configure.
fn recording() -> (
    Arc<Mutex<Vec<TaskEvent>>>,
    impl Fn(TaskEvent) + Send + Sync + 'static,
) {
    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    (events, move |event| sink.lock().unwrap().push(event))
}

fn count(events: &Arc<Mutex<Vec<TaskEvent>>>) -> usize {
    events.lock().unwrap().len()
}

fn ids(events: &Arc<Mutex<Vec<TaskEvent>>>) -> Vec<String> {
    events
        .lock()
        .unwrap()
        .iter()
        .map(|event| event.task_id.clone())
        .collect()
}

/// Lets the timer tasks a paused clock woke run before the test counts their events.
async fn settle() {
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn fetch_events_fire_once_per_interval_after_start() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits::default()));
    let (events, handler) = recording();
    fetch.configure(FetchConfig::default(), handler).unwrap();
    fetch.start().unwrap();
    settle().await;

    // The first event waits a full interval; a partial one is not enough.
    tokio::time::advance(14 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 0);
    tokio::time::advance(MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);

    tokio::time::advance(15 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 2);

    let recorded = events.lock().unwrap();
    for event in recorded.iter() {
        assert_eq!(event.task_id, FETCH_TASK_ID);
        assert!(!event.timed_out);
        assert!(event.input.is_empty());
    }
}

#[tokio::test(start_paused = true)]
async fn stop_halts_fetch_and_cancels_pending_tasks() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits::default()));
    let (events, handler) = recording();
    fetch.configure(FetchConfig::default(), handler).unwrap();
    fetch.start().unwrap();
    fetch
        .schedule_task(TaskConfig {
            task_id: "pending".to_owned(),
            delay: 5 * MINUTE,
            ..TaskConfig::default()
        })
        .unwrap();
    settle().await;

    assert_eq!(fetch.stop(None), Ok(true));
    tokio::time::advance(60 * MINUTE).await;
    settle().await;
    // Neither the fetch timer nor the pending task ever fires again.
    assert_eq!(count(&events), 0);
    assert_eq!(fetch.stop(None), Ok(false));
}

#[tokio::test(start_paused = true)]
async fn stop_task_cancels_one_scheduled_task_only() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits::default()));
    let (events, handler) = recording();
    fetch.configure(FetchConfig::default(), handler).unwrap();
    for task_id in ["a", "b"] {
        fetch
            .schedule_task(TaskConfig {
                task_id: task_id.to_owned(),
                delay: 5 * MINUTE,
                ..TaskConfig::default()
            })
            .unwrap();
    }
    settle().await;

    assert_eq!(fetch.stop(Some("a")), Ok(true));
    tokio::time::advance(5 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["b"]);
    assert_eq!(fetch.stop(Some("unknown")), Ok(false));

    // "a" stays cancelled across its own deadline.
    tokio::time::advance(5 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["b"]);
}

#[tokio::test(start_paused = true)]
async fn a_task_fires_after_its_delay_and_a_periodic_repeats() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits::default()));
    let (events, handler) = recording();
    fetch.configure(FetchConfig::default(), handler).unwrap();
    fetch
        .schedule_task(TaskConfig {
            task_id: "once".to_owned(),
            delay: 10 * MINUTE,
            ..TaskConfig::default()
        })
        .unwrap();
    fetch
        .schedule_task(TaskConfig {
            task_id: "each4".to_owned(),
            delay: 4 * MINUTE,
            periodic: true,
            ..TaskConfig::default()
        })
        .unwrap();
    settle().await;

    // The periodic fires once per delay; each advance crosses one deadline only.
    tokio::time::advance(4 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["each4"]);
    tokio::time::advance(4 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["each4", "each4"]);

    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["each4", "each4", "once"]);
    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["each4", "each4", "once", "each4"]);

    for _ in 0..3 {
        tokio::time::advance(4 * MINUTE).await;
        settle().await;
    }
    let recorded = ids(&events);
    assert_eq!(
        recorded.iter().filter(|id| *id == "once").count(),
        1,
        "a one-off task fires exactly once"
    );
    assert_eq!(recorded.iter().filter(|id| *id == "each4").count(), 6);
}

#[test]
fn finish_is_ok_even_for_an_unknown_task() {
    let backend = FakeBackend::new();
    let fetch = BackgroundFetch::with_backend(backend.clone());
    fetch.finish("x").unwrap();
    assert_eq!(backend.finishes(), 1);
}

#[test]
fn reconfigure_replaces_the_task_handler() {
    let backend = FakeBackend::new();
    let fetch = BackgroundFetch::with_backend(backend.clone());
    let (first_events, first) = recording();
    fetch.configure(FetchConfig::default(), first).unwrap();
    let (second_events, second) = recording();
    fetch.configure(FetchConfig::default(), second).unwrap();
    assert_eq!(backend.configures(), 2);

    backend.emit(FETCH_TASK_ID, false, Vec::new());
    assert_eq!(count(&first_events), 0);
    assert_eq!(count(&second_events), 1);
    assert_eq!(second_events.lock().unwrap()[0].task_id, FETCH_TASK_ID);
}

#[tokio::test(start_paused = true)]
async fn the_fetch_interval_floors_to_the_platform_minimum() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits {
        min_fetch_interval: 15 * MINUTE,
    }));
    let (events, handler) = recording();
    fetch
        .configure(
            FetchConfig {
                minimum_fetch_interval: MINUTE,
                ..FetchConfig::default()
            },
            handler,
        )
        .unwrap();
    fetch.start().unwrap();
    settle().await;

    // The floor replaces the configured minute.
    tokio::time::advance(14 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 0);
    tokio::time::advance(MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);

    // The next minute adds none: the period is the floored 15 minutes.
    tokio::time::advance(MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);
}

#[test]
fn a_status_query_answers_available_on_a_native_target() {
    let fetch = BackgroundFetch::with_backend(LocalBackend::new(Limits::default()));
    assert_eq!(fetch.status(), Ok(Status::Available));

    let (_, handler) = recording();
    fetch.configure(FetchConfig::default(), handler).unwrap();
    assert_eq!(fetch.status(), Ok(Status::Available));
}
