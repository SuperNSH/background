//! The pinned error tests: the code table, the argument and configure gates, and the
//! unsupported-target semantics.

use std::{future::ready, sync::Arc, time::Duration};

use nsh_background::{
    Backend, BackendFuture, BackgroundError, BackgroundFetch, CancelBy, ErrorKind, FakeBackend,
    FetchConfig, OneOffOptions, PeriodicOptions, Status, TaskConfig, TaskEvent, TaskHandler,
    WorkInfo, WorkKind, WorkRequest, Workmanager,
};

/// The pinned unsupported table, as a local stub. The crate's own `UnsupportedBackend` is
/// line-audited against exactly these answers.
struct StubUnsupported;

impl Backend for StubUnsupported {
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

fn one_off_request() -> WorkRequest {
    WorkRequest {
        kind: WorkKind::OneOff,
        unique_name: "sync-work".to_owned(),
        task_name: "sync".to_owned(),
        input: Vec::new(),
        initial_delay: None,
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

#[test]
fn every_kind_has_its_code_and_a_message() {
    let table = [
        (
            ErrorKind::NotAvailable,
            "ERR_BACKGROUND_NOT_AVAILABLE",
            "Background execution is not available on this target.",
        ),
        (
            ErrorKind::NotConfigured,
            "ERR_BACKGROUND_NOT_CONFIGURED",
            "Configure the scheduler with a task handler before starting it.",
        ),
        (
            ErrorKind::InvalidArgument,
            "ERR_BACKGROUND_INVALID_ARGUMENT",
            "An argument is empty or not allowed on this platform.",
        ),
        (
            ErrorKind::Backend,
            "ERR_BACKGROUND_BACKEND",
            "The platform's background scheduler failed.",
        ),
    ];
    for (kind, code, message) in table {
        assert_eq!(kind.code(), code);
        assert_eq!(kind.to_string(), code);
        assert!(!kind.message().is_empty());
        assert_eq!(kind.message(), message);

        let error = BackgroundError::new(kind);
        assert_eq!(error.kind(), kind);
        assert_eq!(error.code(), code);
        assert_eq!(error.status(), None);
        assert_eq!(error.to_string(), message);

        // with_status round-trips: the status rides along, the kind and code stay.
        let carried = error.with_status(7);
        assert_eq!(carried.kind(), kind);
        assert_eq!(carried.code(), code);
        assert_eq!(carried.status(), Some(7));
        assert_eq!(carried.to_string(), message);
    }
}

#[test]
fn empty_or_whitespace_ids_are_refused_without_touching_the_backend() {
    let backend = FakeBackend::new();
    let fetch = BackgroundFetch::with_backend(backend.clone());
    // schedule_task checks ids only after the configure gate, so configure first.
    fetch.configure(FetchConfig::default(), |_| {}).unwrap();
    let error = fetch
        .schedule_task(TaskConfig {
            task_id: "  ".to_owned(),
            ..TaskConfig::default()
        })
        .unwrap_err();
    assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");

    let work = Workmanager::with_backend(backend.clone());
    let error = work
        .register_one_off_task("", "", OneOffOptions::default())
        .unwrap_err();
    assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");
    let error = work
        .register_periodic_task(" ", " ", PeriodicOptions::default())
        .unwrap_err();
    assert_eq!(error.code(), "ERR_BACKGROUND_INVALID_ARGUMENT");

    assert_eq!(backend.schedules(), 0);
    assert_eq!(backend.registers(), 0);
}

#[test]
fn start_before_configure_is_refused() {
    let backend = FakeBackend::new();
    let fetch = BackgroundFetch::with_backend(backend.clone());
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

    // status, stop and finish still resolve.
    assert_eq!(fetch.status(), Ok(Status::Available));
    assert_eq!(fetch.stop(None), Ok(false));
    fetch.finish("any").unwrap();
    assert_eq!(backend.stops(), 1);
    assert_eq!(backend.finishes(), 1);
}

#[tokio::test]
async fn the_unsupported_semantics_answer_plainly_and_fail_coded() {
    // The pinned table, at the backend seam.
    let backend = StubUnsupported;
    assert_eq!(backend.status().await, Ok(Status::Restricted));
    assert_eq!(
        backend
            .configure(FetchConfig::default(), Arc::new(|_: TaskEvent| {}))
            .await
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    assert_eq!(
        backend.start().await.unwrap_err().code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    assert_eq!(backend.stop(None).await, Ok(false));
    assert_eq!(backend.stop(Some("any")).await, Ok(false));
    assert_eq!(
        backend
            .schedule_task(TaskConfig {
                task_id: "sync".to_owned(),
                delay: Duration::from_secs(60),
                ..TaskConfig::default()
            })
            .await
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    backend.finish("any").await.unwrap();
    assert_eq!(
        backend
            .register(one_off_request())
            .await
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    assert_eq!(backend.is_scheduled("any").await, Ok(false));
    assert_eq!(backend.work_info("any").await, Ok(None));
    for cancel_by in [
        CancelBy::UniqueName("u".to_owned()),
        CancelBy::Tag("t".to_owned()),
        CancelBy::All,
    ] {
        backend.cancel(cancel_by).await.unwrap();
    }

    // The same table through the app-facing types.
    let fetch = BackgroundFetch::with_backend(StubUnsupported);
    assert_eq!(fetch.status(), Ok(Status::Restricted));
    assert_eq!(
        fetch
            .configure(FetchConfig::default(), |_| {})
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    // configure never landed, so the scheduler's own gate answers before the backend can.
    assert_eq!(
        fetch.start().unwrap_err().code(),
        "ERR_BACKGROUND_NOT_CONFIGURED"
    );
    assert_eq!(fetch.stop(None), Ok(false));
    fetch.finish("any").unwrap();

    let work = Workmanager::with_backend(StubUnsupported);
    assert_eq!(
        work.register_one_off_task("u", "t", OneOffOptions::default())
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    assert_eq!(
        work.register_periodic_task("u", "t", PeriodicOptions::default())
            .unwrap_err()
            .code(),
        "ERR_BACKGROUND_NOT_AVAILABLE"
    );
    assert_eq!(work.is_scheduled_by_unique_name("u"), Ok(false));
    assert_eq!(work.get_work_info("u").unwrap(), None);
    work.cancel_by_unique_name("u").unwrap();
    work.cancel_by_tag("t").unwrap();
    work.cancel_all().unwrap();
}
