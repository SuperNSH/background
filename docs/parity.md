# Parity

How our API maps onto the three references, one section each. Sources: `.refs/flutter_background_fetch/lib/background_fetch.dart`, `.refs/react-native-background-fetch/index.js` and `.refs/react-native-background-fetch/src/NativeBackgroundFetch.ts`, `.refs/flutter_workmanager/workmanager/lib/src/workmanager_impl.dart` plus the types in `.refs/flutter_workmanager/workmanager_platform_interface/`.

Row terms: **mirrored** means the name exists here and does the analogous thing on the in-process engine; **accepted, no effect yet** means the field is parsed and recorded but the engine ignores it; **not carried** means the API does not exist here, with the reason.

## flutter_background_fetch

| Reference API | Ours |
|---|---|
| `configure(BackgroundFetchConfig, onFetch, [onTimeout])` | Mirrored. Registers the fetch and timeout handlers. The fetch handler is also the dispatcher for every scheduled task — workmanager's `initialize`/`executeTask` split does not exist here. |
| `start()` | Mirrored. Starts the fetch task after `configure` has registered a handler. |
| `stop([taskId])` | Mirrored: one task by id, all tasks when the id is omitted. Returns `bool` (RN's shape, not Dart's `int`). |
| `status` | Mirrored. Reports `STATUS_RESTRICTED`, `STATUS_DENIED` or `STATUS_AVAILABLE`. |
| `scheduleTask(TaskConfig)` | Mirrored. One-shot by default, repeating with `periodic`, `delay` in milliseconds. |
| `finish(taskId)` | Mirrored. Signals the task complete so the engine can mark it done. |
| `registerHeadlessTask(callback)` | Not carried. It exists to run Dart after the app's process dies; NSH apps are one in-process native runtime. |
| `STATUS_RESTRICTED` / `STATUS_DENIED` / `STATUS_AVAILABLE` | Mirrored, same values. |
| `FETCH_RESULT_NEW_DATA` / `FETCH_RESULT_NO_DATA` / `FETCH_RESULT_FAILED` | Not carried. The Dart API's own `finish(taskId)` takes no result, so the constants have no input to feed. |
| (implicit: the default fetch task's id, owned by the reference's native side) | Mirrored as the `FETCH_TASK_ID` constant — the task id the engine hands the configure handler for each fetch event. |
| `NetworkType` (NONE, ANY, UNMETERED, NOT_ROAMING, CELLULAR) | Mirrored as the one `NetworkType` enum; workmanager's variant names map onto it (see Deviations). |
| `TaskType` (DEFAULT, HEALTH_RESEARCH) | Mirrored. Carried by `TaskConfig.type`. |
| `HeadlessEvent` (`taskId`, `timeout`), with the deprecated `HeadlessTask` alias | Mirrored as `TaskEvent` (`taskId`, `timedOut`). The alias is not carried. |
| `BackgroundFetchConfig` | Mirrored as `FetchConfig`. `minimumFetchInterval` drives the fetch period, clamped to the per-target floor (15 minutes on apple and android, none on windows and linux). |
| `TaskConfig` | Mirrored. `taskId`, `delay`, `periodic` and `type` are carried; the reference-only fields in the next row are accepted and recorded. |
| Android-only fields on both configs (`stopOnTerminate`, `startOnBoot`, `enableHeadless`, `forceAlarmManager`, `requiredNetworkType`, `requiresBatteryNotLow`, `requiresStorageNotLow`, `requiresCharging`, `requiresDeviceIdle`) and the iOS-only `requiresNetworkConnectivity` | Accepted and recorded, no effect yet. They describe OS-scheduler behavior, and the engine in process has no scheduler to hand them to. |

## react-native-background-fetch

Same plugin, JS wrapper over the native module (`index.js`, `src/NativeBackgroundFetch.ts`). Its API is the background_fetch API above; only the JS-specific surface differs.

| Reference API | Ours |
|---|---|
| `configure(config, onEvent, onTimeout)` | Mirrored, same handler contract as the Flutter side. |
| `start()` | Mirrored. |
| `stop(taskId)` | Mirrored. Returns `bool`, the native spec's own shape (`Promise<boolean>`). |
| `finish(taskId)` | Mirrored. |
| `status([callback])` | Mirrored; same three `STATUS_*` values. |
| `scheduleTask(config)` | Mirrored. |
| `registerHeadlessTask(task)` | Not carried. AppRegistry headless tasks run JS after the RN process dies; NSH apps are one in-process native runtime. |
| `STATUS_RESTRICTED` / `STATUS_DENIED` / `STATUS_AVAILABLE` | Mirrored. |
| `NETWORK_TYPE_NONE` / `_ANY` / `_UNMETERED` / `_NOT_ROAMING` / `_CELLULAR` | Mirrored; the five constants are the `NetworkType` enum's values. |
| Expo config plugin (`app.plugin.js`, `expo/plugin`) | Not carried. It wires the native module into an Expo-managed build; an NSH app embeds the native module itself. |
| TurboModule `Spec` emitter stubs (`addListener`, `removeListeners`) | Not carried. They exist only for RN's `NativeEventEmitter`; our events reach TSX by pull: the native module queues them and the TS backend awaits `nextEvent` in a loop (see Known gaps). |

## flutter_workmanager

`workmanager/lib/src/workmanager_impl.dart`; types from `workmanager_platform_interface`.

| Reference API | Ours |
|---|---|
| `initialize(callbackDispatcher)` | Not carried. Flutter dispatcher plumbing — `configure`'s handler is the dispatcher. |
| `executeTask(handler, {onTaskStopped})` | Not carried, same plumbing. `BackgroundTaskStoppedHandler` and `StopReason` go with it: a task stopped by the platform is not a concept an in-process engine has. |
| `Workmanager.iOSBackgroundTask` | Not carried. A magic `taskName` the iOS dispatcher receives; our handler gets real task ids, `FETCH_TASK_ID` for the fetch event. |
| `registerOneOffTask(uniqueName, taskName, ...)` | Mirrored. `uniqueName`, `taskName`, `inputData`, `initialDelay`, `tag` and `existingWorkPolicy` are carried — `inputData` is an object of booleans, numbers and strings in Rust, TS and the native bridge (`NativeOneOffOptions.input` in `crates/nsh-background/src/native.rs`, a `JsonValue`, whole numbers becoming integers; nested values are refused with `ERR_BACKGROUND_INVALID_ARGUMENT`) and comes back on the event the work fires. Tests: `input_data_keeps_booleans_numbers_and_strings`, `input_data_that_is_not_flat_is_refused`, `work_options_carry_their_input`, `an_event_reads_back_with_its_input_as_an_object` (unverified until CI runs them). `constraints` and the backoff arguments are accepted and recorded (rows below); `outOfQuotaPolicy`, `foregroundServiceConfig` and `expedited` are not carried (rows below). |
| `registerPeriodicTask(uniqueName, taskName, ...)` | Mirrored, same carried set — `inputData` the same way through `NativePeriodicOptions.input`. `frequency` is clamped to the per-target floor (15 minutes on apple and android, none on windows and linux); `flexInterval` is accepted and recorded, no effect yet. |
| `registerProcessingTask` | Not carried. An iOS `BGProcessingTaskRequest` wrapper; nothing drives it without the Apple scheduler integration. |
| `registerHealthResearchTask` | Not carried. iOS 17+ `BGHealthResearchTaskRequest`; same reason. |
| `registerContinuedProcessingTask` | Not carried. iOS 26+ `BGContinuedProcessingTaskRequest`; same reason. |
| `cancelByUniqueName(uniqueName)` | Mirrored. |
| `cancelByTag(tag)` | Mirrored. |
| `cancelAll()` | Mirrored. |
| `isScheduledByUniqueName(uniqueName)` | Mirrored. True while the task is enqueued or running — the reference's documented meaning. |
| `getWorkInfo(uniqueName)` | Mirrored. Returns `WorkInfo`, or nothing when no such task exists. |
| `printScheduledTasks()` | Not carried. A debug dump of BGTaskScheduler's queue; `getWorkInfo` is the query surface here. |
| `reportProgress(progress)` | Not carried. Progress reporting for long-running Android foreground-service workers. |
| `setProgressListener(listener)` | Not carried with it; `ProgressListener` would have no source of events. |
| `WorkInfo` | Mirrored: `uniqueName`, `state`, `isPeriodic`, `taskName`, `tags`. `lastFinishedAt` is not carried — the reference itself leaves it empty on Android because WorkManager exposes no finish timestamp. |
| `WorkState` (scheduled, running, succeeded, failed, cancelled) | Mirrored, same variants. |
| `Constraints` | Mirrored: `networkType`, `requiresBatteryNotLow`, `requiresCharging`, `requiresDeviceIdle`, `requiresStorageNotLow`. Gating is accepted and recorded, no effect yet — the engine runs a task when it comes due, constraint checks arrive with the OS schedulers. `contentUriTriggers` is not carried (row below). |
| `ContentUriTrigger` | Not carried. Android content-URI observation; meaningless without the OS scheduler. |
| `ExistingWorkPolicy` (append, keep, replace, update) | Mirrored. `Append` behaves as `Replace` for now — an in-process engine has no work chain to append to (see Deviations). |
| `ExistingPeriodicWorkPolicy` (keep, replace, update) | Mirrored; periodic registration takes the same policy set. |
| `BackoffPolicy` (exponential, linear) | Mirrored; accepted and recorded, no effect yet. |
| `NetworkType` (connected, metered, notRequired, notRoaming, unmetered, temporarilyUnmetered) | Mapped onto the one `NetworkType` enum; the exact mapping is in Deviations. |
| `OutOfQuotaPolicy` | Not carried. Quota fallback for Android expedited jobs, and `expedited` is not carried either. |
| `ForegroundServiceConfig` (+ `ForegroundServiceType`) | Not carried. Promotes an Android worker to a foreground service with a notification; Android integration is pending. |
| `expedited` (flag on `registerOneOffTask`) | Not carried. Android expedited work; there is no OS scheduler to expedite into. |
| Platform-interface plumbing (`WorkmanagerPlatform`, `TaskStatus`, Pigeon `WorkmanagerHostApi` / `WorkmanagerFlutterApi`) | Not carried. Flutter plugin internals and a debug-broadcast enum; a native crate has no counterpart. |

## Error contract

Every failure is one of these kinds. Codes and messages are stable.

| Kind | Code | Message |
|---|---|---|
| `NotAvailable` | `ERR_BACKGROUND_NOT_AVAILABLE` | Background execution is not available on this target. |
| `NotConfigured` | `ERR_BACKGROUND_NOT_CONFIGURED` | Configure the scheduler with a task handler before starting it. |
| `InvalidArgument` | `ERR_BACKGROUND_INVALID_ARGUMENT` | An argument is empty or not allowed on this platform. |
| `Backend` | `ERR_BACKGROUND_BACKEND` | The platform's background scheduler failed. |

## Deviations

- One `NetworkType` enum, background_fetch's five values. workmanager's names map onto it: `connected` -> `Any`, `notRequired` -> `None`, `notRoaming` -> `NotRoaming`, `unmetered` -> `Unmetered`, `temporarilyUnmetered` -> `Unmetered`, `metered` -> `Cellular`.
- The TypeScript classes are instances, not statics. Both fetch references expose statics; workmanager's Dart class is already an instance.
- `stop` returns `bool` — RN's shape, not Dart's `int`.
- `ExistingWorkPolicy::Append` behaves as `Replace` while the engine is in-process.

## Verification

What can be verified where. CI is configured to compile every platform file — the `cross-check` matrix cross-clippies `aarch64-apple-ios-sim`, `aarch64-linux-android`, `wasm32-unknown-unknown` and `x86_64-pc-windows-gnu`, and the `rust` job compiles `linux.rs` natively — and run 44381445909 on `master` (https://github.com/SuperNSH/background/actions/runs/44381445909) passed all six jobs: `rust`, `typescript` and the four cross-check entries. Rows are verified by that run only to the extent a job compiles or runs them; the bullets below say which. No claim of local verification outruns this list.

- `apple.rs` — compiles and tests on the macOS host (`target_vendor = "apple"` is the host's own cfg).
- `windows.rs` — cross-clippies on the host under `x86_64-pc-windows-gnu`, and in CI through the matrix's `x86_64-pc-windows-gnu` entry once the matrix has run.
- `wasm32-unknown-unknown` — cross-clippies on the host; its cfg selects `UnsupportedBackend`.
- `linux.rs` — CI only: compiled natively by the ubuntu-latest `rust` job.
- `android.rs` — CI only: compiled by the `cross-check` matrix's `aarch64-linux-android` entry, with `tests/android.rs` (`--all-targets`)..
- `tests/windows.rs` — same, compiled by the `x86_64-pc-windows-gnu` entry. Both are compile-only: no runner executes them, so their assertions are unexercised (a deliberately wrong assertion passed that matrix on run 44381174285).
- `aarch64-unknown-linux-gnu` — not checkable on the host. The rustup target is installed, but `nsh-core` -> `rustls` -> `ring` needs a C cross-compiler the machine does not have; ring's build script fails.

## Known gaps

- Native-to-TSX event delivery is by pull, and unverified until CI runs it. NSH 0.6.0's `Callback` (`nsh-core/src/element/callback.rs`) is `Rc`-based and posts only from the engine thread, so it cannot be captured by the crate's `Send + Sync` handler; instead `configure` queues every event in the crate (`Events` in `native.rs`) and the TS backend (`packages/nsh-background/src/rust.ts`) awaits `nextEvent` in a loop. A `stop` with no task id clears the queue and resolves the waiting read `null`, which ends the loop; the next `configure` or `start` begins a new one. Rust tests: `events_come_out_in_the_order_the_handler_got_them`, `a_read_waits_for_an_event_that_comes_later`, `a_stop_ends_the_waiting_read_and_clears_the_queue`, `the_configured_handler_queues_what_the_backend_fires`; TS: `tests/rust.test.ts`. No test fires a real timer through the crate from TSX.
- OS-level scheduling — BGTaskScheduler, WorkManager, Windows `BackgroundTaskBuilder` — is pending per-platform integration. While the process lives, the in-process engine is the behavior.
- Web is an unsupported target and fails with the coded `NotAvailable` error.
