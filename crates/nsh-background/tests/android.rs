//! Android's bridge through the public API: available, and under WorkManager's 15-minute
//! floor. Compiled only under an Android target; no runner executes it.
#![cfg(target_os = "android")]

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use nsh_background::{BackgroundFetch, FetchConfig, Status, TaskEvent};

const MINUTE: Duration = Duration::from_secs(60);

#[tokio::test(start_paused = true)]
async fn android_answers_available_and_floors_the_fetch_interval() {
    let fetch = BackgroundFetch::new();
    assert_eq!(fetch.status(), Ok(Status::Available));

    let events: Arc<Mutex<Vec<TaskEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    fetch
        .configure(
            FetchConfig {
                minimum_fetch_interval: MINUTE,
                ..FetchConfig::default()
            },
            move |event| sink.lock().unwrap().push(event),
        )
        .unwrap();
    fetch.start().unwrap();
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }

    tokio::time::advance(14 * MINUTE).await;
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    assert_eq!(events.lock().unwrap().len(), 0);
    tokio::time::advance(MINUTE).await;
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
    assert_eq!(events.lock().unwrap().len(), 2);
}
