//! The pinned work tests: registration, the policies, cancels, and what queries answer.

use std::{
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use nsh_background::{
    Backend, FakeBackend, FetchConfig, InputValue, Limits, LocalBackend, OneOffOptions,
    PeriodicOptions, TaskEvent, TaskHandler, WorkState, Workmanager,
};

const MINUTE: Duration = Duration::from_secs(60);

/// The recorded events and a handler that records into them; one per configure. The
/// Backend trait takes the handler as a TaskHandler, not a bare closure.
fn recording() -> (Arc<Mutex<Vec<TaskEvent>>>, TaskHandler) {
    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    (
        events,
        Arc::new(move |event: TaskEvent| sink.lock().unwrap().push(event)),
    )
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

/// A manager over a configured engine: LocalBackend is not Clone, so the handler is
/// installed through the Backend trait before the backend moves into the manager.
async fn manager() -> (Workmanager, Arc<Mutex<Vec<TaskEvent>>>) {
    let backend = LocalBackend::new(Limits::default());
    let (events, handler) = recording();
    backend
        .configure(FetchConfig::default(), handler)
        .await
        .unwrap();
    (Workmanager::with_backend(backend), events)
}

#[tokio::test(start_paused = true)]
async fn a_one_off_task_runs_after_its_initial_delay_then_succeeds() {
    let backend = LocalBackend::new(Limits::default());
    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    // The handler reads the record mid-event; Running lasts only inside the handler call,
    // so this is the one place a test can see it.
    let slot = Arc::new(OnceLock::<Workmanager>::new());
    let states: Arc<Mutex<Vec<WorkState>>> = Arc::new(Mutex::new(Vec::new()));
    let (handler_events, handler_slot, handler_states) =
        (events.clone(), slot.clone(), states.clone());
    backend
        .configure(
            FetchConfig::default(),
            Arc::new(move |event: TaskEvent| {
                handler_events.lock().unwrap().push(event);
                if let Some(work) = handler_slot.get() {
                    let state = work.get_work_info("sync-work").unwrap().unwrap().state;
                    handler_states.lock().unwrap().push(state);
                }
            }),
        )
        .await
        .unwrap();
    let work = Workmanager::with_backend(backend);
    // set runs once, before any timer can fire, so it cannot find the slot taken.
    let _ = slot.set(work.clone());

    work.register_one_off_task(
        "sync-work",
        "sync",
        OneOffOptions {
            input: vec![("count".to_owned(), InputValue::Int(3))],
            initial_delay: Some(2 * MINUTE),
            ..OneOffOptions::default()
        },
    )
    .unwrap();

    // Scheduled until its delay passes.
    let before = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(before.state, WorkState::Scheduled);
    assert!(!before.is_periodic);
    assert_eq!(work.is_scheduled_by_unique_name("sync-work"), Ok(true));

    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);
    let event = events.lock().unwrap()[0].clone();
    assert_eq!(event.task_id, "sync");
    assert!(!event.timed_out);
    assert_eq!(event.input, vec![("count".to_owned(), InputValue::Int(3))]);
    assert_eq!(*states.lock().unwrap(), [WorkState::Running]);

    let after = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(after.state, WorkState::Succeeded);
    assert_eq!(work.is_scheduled_by_unique_name("sync-work"), Ok(false));

    // A one-off never fires twice.
    tokio::time::advance(30 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);
}

#[tokio::test(start_paused = true)]
async fn a_periodic_task_reschedules_and_reports_scheduled_between_runs() {
    let backend = LocalBackend::new(Limits::default());
    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    // is_scheduled read mid-event: Running counts as scheduled too.
    let slot = Arc::new(OnceLock::<Workmanager>::new());
    let scheduled: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));
    let (handler_events, handler_slot, handler_scheduled) =
        (events.clone(), slot.clone(), scheduled.clone());
    backend
        .configure(
            FetchConfig::default(),
            Arc::new(move |event: TaskEvent| {
                handler_events.lock().unwrap().push(event);
                if let Some(work) = handler_slot.get() {
                    let answer = work.is_scheduled_by_unique_name("sync-work").unwrap();
                    handler_scheduled.lock().unwrap().push(answer);
                }
            }),
        )
        .await
        .unwrap();
    let work = Workmanager::with_backend(backend);
    // set runs once, before any timer can fire, so it cannot find the slot taken.
    let _ = slot.set(work.clone());

    work.register_periodic_task(
        "sync-work",
        "sync",
        PeriodicOptions {
            frequency: Some(5 * MINUTE),
            initial_delay: Some(2 * MINUTE),
            ..PeriodicOptions::default()
        },
    )
    .unwrap();

    let before = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(before.state, WorkState::Scheduled);
    assert!(before.is_periodic);
    assert_eq!(work.is_scheduled_by_unique_name("sync-work"), Ok(true));

    // The first firing waits out the initial delay.
    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);
    assert_eq!(events.lock().unwrap()[0].task_id, "sync");
    assert_eq!(*scheduled.lock().unwrap(), [true]);

    // Between runs the record is Scheduled again, and still scheduled to queries.
    let between = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(between.state, WorkState::Scheduled);
    assert_eq!(work.is_scheduled_by_unique_name("sync-work"), Ok(true));

    // The frequency repeats.
    tokio::time::advance(5 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 2);
    assert_eq!(*scheduled.lock().unwrap(), [true, true]);
    let after = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(after.state, WorkState::Scheduled);
    assert_eq!(work.is_scheduled_by_unique_name("sync-work"), Ok(true));
}

#[tokio::test(start_paused = true)]
async fn keep_ignores_a_duplicate_and_replace_replaces_it() {
    let (work, events) = manager().await;
    work.register_one_off_task(
        "sync-work",
        "first",
        OneOffOptions {
            initial_delay: Some(10 * MINUTE),
            ..OneOffOptions::default()
        },
    )
    .unwrap();
    work.register_one_off_task(
        "sync-work",
        "second",
        OneOffOptions {
            initial_delay: Some(MINUTE),
            existing_work_policy: Some(nsh_background::ExistingWorkPolicy::Keep),
            ..OneOffOptions::default()
        },
    )
    .unwrap();

    // Keep leaves the first task: had it taken over, "second" would fire at minute one.
    tokio::time::advance(MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 0);
    assert_eq!(
        work.get_work_info("sync-work").unwrap().unwrap().task_name,
        Some("first".to_owned())
    );

    work.register_one_off_task(
        "sync-work",
        "third",
        OneOffOptions {
            initial_delay: Some(2 * MINUTE),
            existing_work_policy: Some(nsh_background::ExistingWorkPolicy::Replace),
            ..OneOffOptions::default()
        },
    )
    .unwrap();
    // Replace takes the name at once: the record names the new task while it waits.
    assert_eq!(
        work.get_work_info("sync-work").unwrap().unwrap().task_name,
        Some("third".to_owned())
    );

    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["third"]);
    // Neither the replaced original nor the ignored duplicate ever fires.
    tokio::time::advance(10 * MINUTE).await;
    settle().await;
    assert_eq!(ids(&events), ["third"]);
    assert_eq!(
        work.get_work_info("sync-work").unwrap().unwrap().state,
        WorkState::Succeeded
    );
}

#[tokio::test(start_paused = true)]
async fn cancel_by_unique_name_tag_and_all_cancel_exactly_their_set() {
    let (work, events) = manager().await;
    let options = |tag: Option<&str>| OneOffOptions {
        initial_delay: Some(30 * MINUTE),
        tag: tag.map(str::to_owned),
        ..OneOffOptions::default()
    };
    work.register_one_off_task("cleanup", "clean", options(Some("maintenance")))
        .unwrap();
    work.register_one_off_task("sync-a", "sync", options(None))
        .unwrap();
    work.register_one_off_task("sync-b", "sync", options(None))
        .unwrap();

    // By unique name: exactly that one.
    work.cancel_by_unique_name("sync-a").unwrap();
    assert_eq!(
        work.get_work_info("sync-a").unwrap().unwrap().state,
        WorkState::Cancelled
    );
    assert_eq!(work.is_scheduled_by_unique_name("sync-a"), Ok(false));
    for name in ["cleanup", "sync-b"] {
        assert_eq!(
            work.get_work_info(name).unwrap().unwrap().state,
            WorkState::Scheduled
        );
    }

    // By tag: exactly the tagged one.
    work.cancel_by_tag("maintenance").unwrap();
    assert_eq!(
        work.get_work_info("cleanup").unwrap().unwrap().state,
        WorkState::Cancelled
    );
    assert_eq!(
        work.get_work_info("sync-b").unwrap().unwrap().state,
        WorkState::Scheduled
    );

    // All: whatever is left.
    work.cancel_all().unwrap();
    assert_eq!(
        work.get_work_info("sync-b").unwrap().unwrap().state,
        WorkState::Cancelled
    );

    // A cancelled record never fires.
    tokio::time::advance(60 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 0);
}

#[tokio::test(start_paused = true)]
async fn work_info_reports_the_record_or_none_for_an_unknown_name() {
    let (work, _events) = manager().await;
    assert_eq!(work.get_work_info("missing").unwrap(), None);

    work.register_one_off_task(
        "sync-work",
        "sync",
        OneOffOptions {
            initial_delay: Some(30 * MINUTE),
            tag: Some("uploads".to_owned()),
            ..OneOffOptions::default()
        },
    )
    .unwrap();
    let info = work.get_work_info("sync-work").unwrap().unwrap();
    assert_eq!(info.unique_name, "sync-work");
    assert_eq!(info.state, WorkState::Scheduled);
    assert!(!info.is_periodic);
    assert_eq!(info.task_name, Some("sync".to_owned()));
    assert_eq!(info.tags, ["uploads"]);
}

#[tokio::test(start_paused = true)]
async fn recorded_options_are_kept_but_change_nothing_yet() {
    let kept = || OneOffOptions {
        constraints: Some(nsh_background::Constraints {
            required_network_type: Some(nsh_background::NetworkType::Unmetered),
            requires_charging: true,
            ..nsh_background::Constraints::default()
        }),
        backoff_policy: Some(nsh_background::BackoffPolicy::Linear),
        backoff_policy_delay: Some(Duration::from_secs(10)),
        tag: Some("uploads".to_owned()),
        initial_delay: Some(2 * MINUTE),
        ..OneOffOptions::default()
    };

    // The fake keeps the options on the recorded request, exactly as given.
    let fake = FakeBackend::new();
    Workmanager::with_backend(fake.clone())
        .register_one_off_task("sync-work", "sync", kept())
        .unwrap();
    let request = fake.last_request().unwrap();
    let expected = kept();
    assert_eq!(request.constraints, expected.constraints);
    assert_eq!(request.backoff_policy, expected.backoff_policy);
    assert_eq!(request.backoff_policy_delay, expected.backoff_policy_delay);
    assert_eq!(request.tag, expected.tag);
    assert_eq!(fake.registers(), 1);

    // The engine's timing ignores them: the work runs exactly at its delay anyway.
    let (work, events) = manager().await;
    work.register_one_off_task("sync-work", "sync", kept())
        .unwrap();
    tokio::time::advance(2 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1);
    assert_eq!(events.lock().unwrap()[0].task_id, "sync");
    tokio::time::advance(30 * MINUTE).await;
    settle().await;
    assert_eq!(count(&events), 1, "no retries and no constraint gating");
}
