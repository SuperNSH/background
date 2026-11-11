//! What the crate's own tests share: an event recorder and a paused-time settle step.

use std::sync::{Arc, Mutex};

use crate::backend::TaskEvent;

/// The recorded events, and the handler that records into them.
pub(crate) type Recording = (
    Arc<Mutex<Vec<TaskEvent>>>,
    Arc<dyn Fn(TaskEvent) + Send + Sync>,
);

/// A handler that records every event it receives, for tests to count and inspect.
pub(crate) fn recording() -> Recording {
    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    (
        events,
        Arc::new(move |event: TaskEvent| {
            sink.lock().unwrap().push(event);
        }),
    )
}

/// Lets the woken timer tasks of a paused clock run before the test looks at the events.
pub(crate) async fn settle() {
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
}
