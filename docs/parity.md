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
| TurboModule `Spec` emitter stubs (`addListener`, `removeListeners`) | Not carried. They exist only for RN's `NativeEventEmitter`; our events flow through the NSH bridge (pending, see Known gaps). |

## flutter_workmanager

`workmanager/lib/src/workmanager_impl.dart`; types from `workmanager_platform_interface`.

| Reference API | Ours |
|---|---|
| `initialize(callbackDispatcher)` | Not carried. Flutter dispatcher plumbing — `configure`'s handler is the dispatcher. |
| `executeTask(handler, {onTaskStopped})` | Not carried, same plumbing. `BackgroundTaskStoppedHandler` and `StopReason` go with it: a task stopped by the platform is not a concept an in-process engine has. |
| `Workmanager.iOSBackgroundTask` | Not carried. A magic `taskName` the iOS dispatcher receives; our handler gets real task ids, `FETCH_TASK_ID` for the fetch event. |
| `registerOneOffTask(uniqueName, taskName, ...)` | Mirrored. `uniqueName`, `taskName`, `inputData`, `initialDelay`, `tag` and `existingWorkPolicy` are carried — `inputData` by the Rust and TS options only: the native bridge options (`NativeOneOffOptions` in `crates/nsh-background/src/native.rs`) expose no input field in v0.1, so work registered from TSX starts with empty input. `constraints` and the backoff arguments are accepted and recorded (rows below); `outOfQuotaPolicy`, `foregroundServiceConfig` and `expedited` are not carried (rows below). |
| `registerPeriodicTask(uniqueName, taskName, ...)` | Mirrored, same carried set — `inputData` with the same caveat: `NativePeriodicOptions` exposes no input field in v0.1 either. `frequency` is clamped to the per-target floor (15 minutes on apple and android, none on windows and linux); `flexInterval` is accepted and recorded, no effect yet. |
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

What can be verified where. CI is configured to compile every platform file — the `cross-check` matrix cross-clippies `aarch64-apple-ios-sim`, `aarch64-linux-android`, `wasm32-unknown-unknown` and `x86_64-pc-windows-gnu`, and the `rust` job compiles `linux.rs` natively — but no push has run the four-entry matrix yet, so it is unverified until a run is green (AGENTS.md: a claim with no passing run is unverified). No claim of local verification outruns this list.

- `apple.rs` — compiles and tests on the macOS host (`target_vendor = "apple"` is the host's own cfg).
- `windows.rs` — cross-clippies on the host under `x86_64-pc-windows-gnu`, and in CI through the matrix's `x86_64-pc-windows-gnu` entry once the matrix has run.
- `wasm32-unknown-unknown` — cross-clippies on the host; its cfg selects `UnsupportedBackend`.
- `linux.rs` — CI only: compiled natively by the ubuntu-latest `rust` job.
- `android.rs` — CI only: compiled by the `cross-check` matrix's `aarch64-linux-android` entry.
- `aarch64-unknown-linux-gnu` — not checkable on the host. The rustup target is installed, but `nsh-core` -> `rustls` -> `ring` needs a C cross-compiler the machine does not have; ring's build script fails.

## Known gaps

- Native-to-TSX event delivery is pending an NSH bridge. The native module's `configure` drops events today; the TS class keeps its callbacks for when the bridge lands.
- OS-level scheduling — BGTaskScheduler, WorkManager, Windows `BackgroundTaskBuilder` — is pending per-platform integration. While the process lives, the in-process engine is the behavior.
- Web is an unsupported target and fails with the coded `NotAvailable` error.
