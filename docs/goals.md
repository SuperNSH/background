# Background tasks: goals

What `nsh-background` is being built to be, what CI must show for it to count, the pinned error contract, and every edge case with the test that covers it. The research that led here is in [brief.md](brief.md), each choice and its reason in [decisions.md](decisions.md), per-name status in [parity.md](parity.md).

## Goals

Two deliverables with one name and one behavior:

- The Rust crate `nsh-background` in `crates/nsh-background/`, giving NSH apps background-task scheduling with the API names and behavior of `flutter_background_fetch`, `react-native-background-fetch` and `flutter_workmanager`:
  - the fetch side: `configure` with `onEvent`/`onTimeout`, `start`, `stop` (all tasks or one id), `scheduleTask`, `finish`, `status`;
  - the work side: `registerOneOffTask`, `registerPeriodicTask`, `cancelByUniqueName`, `cancelByTag`, `cancelAll`, `isScheduledByUniqueName`, `getWorkInfo`;
  - the references' constants: `STATUS_RESTRICTED` 0, `STATUS_DENIED` 1, `STATUS_AVAILABLE` 2, `NETWORK_TYPE_NONE` through `NETWORK_TYPE_CELLULAR` 0-4, and `FETCH_TASK_ID`, the id the default fetch task carries.
- The TypeScript package `nsh-background` in `packages/nsh-background/`, the same surface in camelCase, tested through a `memoryBackend` so its tests run without any native backend.

The layout follows SuperNSH/biometrics: one crate, one package, a `Backend` trait with a fake for tests, coded errors in the `ERR_` scheme, docs in `docs/`, and CI on GitHub Actions that compiles and tests everything — nothing builds on a developer machine. Which reference names are carried, accepted or left out is tracked row by row in [parity.md](parity.md).

One in-process engine owns the scheduling behind the `Backend` trait:

- `LocalBackend` runs tasks on timers the process owns, with the platform's floors (such as `min_fetch_interval`) in `Limits`. Desktop and in-process targets are this shape.
- `FakeBackend` scripts backend answers for the tests.
- `UnsupportedBackend` answers the unsupported table below; on `wasm32` targets it is the backend.

The web is an unsupported target. It answers the same table instead of pretending:

| Operation | Answer |
|---|---|
| `status` | Restricted |
| `configure`, `start`, `schedule_task`, `register` | `ERR_BACKGROUND_NOT_AVAILABLE` |
| `stop` | `Ok(false)` |
| `finish`, `is_scheduled`, `work_info`, `cancel` | answer plainly |

## Acceptance criteria

The five whole-tree gates pass in CI on `master`:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --all-targets`
4. `nsh check packages/nsh-background`
5. `nsh test packages/nsh-background`

Behind the gates:

- Every operation resolves exactly once: a success, a coded failure, or a cancel. Nothing hangs.
- Every error the library answers carries one of the four stable codes below. No fifth code, no bare string.
- Parity rows are honest: a row says done only when a CI run has shown it.
- README claims match CI. A claim with no passing run is unverified and does not go in.

## The error enum

Pinned. Rust and TypeScript use the same codes and the same messages, verbatim.

| `ErrorKind` | `code()` | `message()` |
|---|---|---|
| `NotAvailable` | `ERR_BACKGROUND_NOT_AVAILABLE` | Background execution is not available on this target. |
| `NotConfigured` | `ERR_BACKGROUND_NOT_CONFIGURED` | Configure the scheduler with a task handler before starting it. |
| `InvalidArgument` | `ERR_BACKGROUND_INVALID_ARGUMENT` | An argument is empty or not allowed on this platform. |
| `Backend` | `ERR_BACKGROUND_BACKEND` | The platform's background scheduler failed. |

Rust:

- `#[non_exhaustive] enum ErrorKind`, with `code() -> &'static str` and `message() -> &'static str`; `Display` prints the code.
- `#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] struct BackgroundError { kind: ErrorKind, status: Option<i64> }`, with `new(kind)`, `with_status(i64) -> Self` (`#[must_use]`), `kind()`, `status() -> Option<i64>`, `code() -> &'static str`, and `#[error("{}", .kind.message())]` for the display.

TypeScript:

- `type BackgroundErrorCode` is the union of the four codes.
- `class BackgroundError extends Error` with `readonly code`, carrying the same messages as Rust, word for word.
- `asBackgroundError(error: unknown): BackgroundError` keeps a known code, unwraps an `ERR_NSH_HOST_ERROR` rejection whose message starts with a known code in the form `<CODE>: text`, and maps anything else to `ERR_BACKGROUND_BACKEND`.

## Edge cases

One row per test in the pinned inventory; rows 1-18 keep its numbering. Rust integration tests live in `crates/nsh-background/tests/`: they import `nsh_background::*`, run on a tokio current-thread runtime with `tokio::time::pause()`/`advance` driving the timers, use `FakeBackend` for scripted paths and `LocalBackend::new(Limits)` for engine paths. TypeScript tests live in `packages/nsh-background/tests/` (`import { expect, test } from "nsh/test"`), driven through `memoryBackend`. A cell saying none means the inventory pins no test on that side for the case.

| # | Edge case | Rust | TypeScript |
|---|---|---|---|
| 1 | A fetch event fires once per interval after `start` — two intervals give exactly two events, with `task_id` `FETCH_TASK_ID` and `timed_out` false | `fetch.rs` `fetch_events_fire_once_per_interval_after_start` | `fetch.test.ts`: emit routes an event to `onEvent` |
| 2 | `stop(None)` answers true and cancels pending tasks; advancing time yields no further events; a second `stop` answers false | `fetch.rs` `stop_halts_fetch_and_cancels_pending_tasks` | `fetch.test.ts`: `stop` passes through to the backend |
| 3 | `stop(Some(id))` cancels exactly that one scheduled task while the other still fires; stopping an unknown id answers false | `fetch.rs` `stop_task_cancels_one_scheduled_task_only` | `fetch.test.ts`: `stop` passes through to the backend |
| 4 | A one-off task fires once after its delay; a periodic task fires at each delay | `fetch.rs` `a_task_fires_after_its_delay_and_a_periodic_repeats` | `fetch.test.ts`: `scheduleTask` passes through (the timing itself is engine-side) |
| 5 | `finish` answers `Ok(())` even for an unknown task id | `fetch.rs` `finish_is_ok_even_for_an_unknown_task` | `fetch.test.ts`: `finish` passes through to the backend |
| 6 | `reconfigure` replaces the task handler; after it only the new handler receives events | `fetch.rs` `reconfigure_replaces_the_task_handler` | `fetch.test.ts`: `configure` stores the handler |
| 7 | The configured fetch interval floors to the platform minimum: 1 min configured against a 15 min floor gives exactly one event per 15 min, and the next minute adds none | `fetch.rs` `the_fetch_interval_floors_to_the_platform_minimum` | none |
| 8 | A status query answers Available on a native target | `fetch.rs` `a_status_query_answers_available_on_a_native_target` | `fetch.test.ts`: `status` passes through to the backend |
| 9 | A one-off task goes Scheduled -> Running, delivers one event with its task name and input, then Succeeded | `work.rs` `a_one_off_task_runs_after_its_initial_delay_then_succeeds` | `workmanager.test.ts`: `registerOneOffTask` validates names and builds the WorkRequest the backend receives |
| 10 | A periodic task reschedules at its frequency; `is_scheduled` is true while the task is scheduled or running | `work.rs` `a_periodic_task_reschedules_and_reports_scheduled_between_runs` | `workmanager.test.ts`: `registerPeriodicTask` builds the WorkRequest the backend receives |
| 11 | Registering the same unique name twice with Keep leaves the first task; with Replace the new one takes it | `work.rs` `keep_ignores_a_duplicate_and_replace_replaces_it` | none |
| 12 | Cancel by unique name, by tag and cancel-all each cancel exactly their set; a cancelled record reads Cancelled from `work_info` | `work.rs` `cancel_by_unique_name_tag_and_all_cancel_exactly_their_set` | `workmanager.test.ts`: `cancelByUniqueName`/`cancelByTag`/`cancelAll` pass the right `CancelBy` |
| 13 | `work_info` answers the full record for a known name and None for an unknown one | `work.rs` `work_info_reports_the_record_or_none_for_an_unknown_name` | `workmanager.test.ts`: `isScheduledByUniqueName` and `getWorkInfo` answer from the backend |
| 14 | Constraints, backoff and tag are kept on the recorded request (`FakeBackend.last_request`) while the timing ignores them | `work.rs` `recorded_options_are_kept_but_change_nothing_yet` | `workmanager.test.ts`: the WorkRequest the backend receives keeps them |
| 15 | Each of the four kinds answers its exact code and a non-empty message, and `with_status` round-trips | `errors.rs` `every_kind_has_its_code_and_a_message` | `errors.test.ts`: the four codes each carry Rust's message |
| 16 | An empty or whitespace id is refused with `ERR_BACKGROUND_INVALID_ARGUMENT` before the backend is touched; its counters stay zero | `errors.rs` `empty_or_whitespace_ids_are_refused_without_touching_the_backend` | `fetch.test.ts`: invalid args reject INVALID_ARGUMENT with the backend counters at zero; `workmanager.test.ts`: `registerOneOffTask`/`registerPeriodicTask` validate names |
| 17 | `start`/`schedule_task` before `configure` answer `ERR_BACKGROUND_NOT_CONFIGURED`; `status`, `stop` and `finish` still resolve | `errors.rs` `start_before_configure_is_refused` | `fetch.test.ts`: start/scheduleTask before configure reject NOT_CONFIGURED |
| 18 | The unsupported semantics answer plainly and fail coded: status Restricted; `configure`/`start`/`schedule_task`/`register` fail `ERR_BACKGROUND_NOT_AVAILABLE`; `stop` answers `Ok(false)`; `finish`/`is_scheduled`/`work_info`/`cancel` answer plainly. The crate's own `UnsupportedBackend` is line-audited against this same table, and on `wasm32` targets it is the table | `errors.rs` `the_unsupported_semantics_answer_plainly_and_fail_coded` | none in the pinned set |
| 19 | A `timedOut` event routes to `onTimeout`, and the default `onTimeout` finishes the task | none in the pinned set | `fetch.test.ts`: emit routes by `timedOut` to `onTimeout` (default finishes the task) or `onEvent` |
| 20 | `asBackgroundError` keeps a known code, unwraps an `ERR_NSH_HOST_ERROR` rejection whose message starts `<CODE>: text`, and maps anything else to `ERR_BACKGROUND_BACKEND` | none; the unwrap is TypeScript-side | `errors.test.ts` |
| 21 | The public constants keep the references' values: `STATUS_*` 0-2 and `NETWORK_TYPE_*` 0-4; `FETCH_TASK_ID` names the default fetch task | `fetch.rs` row 1: the event's `task_id` is `FETCH_TASK_ID` | `fetch.test.ts`: `STATUS_*`, `NETWORK_TYPE_*` and `FETCH_TASK_ID` match the references' values |
| 22 | The Android platform backend compiles under its target | `tests/android.rs` — only CI's cross-target matrix compiles it; no runner executes it | none |
| 23 | The Windows platform backend compiles under its target | `tests/windows.rs` — only CI's cross-target matrix compiles it; no runner executes it | none |
