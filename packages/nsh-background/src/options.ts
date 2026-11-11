/** Whether background work runs, as `status` reports it: restricted, denied or available. */
export type Status = "restricted" | "denied" | "available";

/** The network a task or a piece of work requires, `background_fetch`'s own five. */
export type NetworkType = "none" | "any" | "unmetered" | "notRoaming" | "cellular";

/** The kind of task a schedule carries. Health research gets the longer iOS budget. */
export type TaskType = "default" | "healthResearch";

/** Where a piece of work is: scheduled, running, or done for good. */
export type WorkState = "scheduled" | "running" | "succeeded" | "failed" | "cancelled";

/** What enqueuing work under a taken name does to the existing work. `append` and `update` both run as `replace` in-process; only `keep` preserves. */
export type ExistingWorkPolicy = "keep" | "replace" | "append" | "update";

/** What re-registering periodic work under a taken name does. `replace` and `update` both replace in-process. */
export type ExistingPeriodicWorkPolicy = "keep" | "update" | "replace";

/** How a failed piece of work waits before its next attempt. */
export type BackoffPolicy = "linear" | "exponential";

/** One value of input data. The references carry no other kind. */
export type InputValue = boolean | number | string;

/** A task's or a piece of work's input data, by name. */
export type Input = Record<string, InputValue>;

/** The references' own numbers for `Status`. */
export const STATUS_RESTRICTED = 0;
export const STATUS_DENIED = 1;
export const STATUS_AVAILABLE = 2;

/** The references' own numbers for `NetworkType`. */
export const NETWORK_TYPE_NONE = 0;
export const NETWORK_TYPE_ANY = 1;
export const NETWORK_TYPE_UNMETERED = 2;
export const NETWORK_TYPE_NOT_ROAMING = 3;
export const NETWORK_TYPE_CELLULAR = 4;

/** The task id each fetch event carries; the references' own value. */
export const FETCH_TASK_ID = "flutter_background_fetch";

/** The settings of the periodic fetch. Unset fields keep the defaults; minutes, not seconds. */
export interface FetchConfig {
  /** The shortest interval between two fetch events, in minutes. 15 by default. */
  minimumFetchIntervalMinutes?: number;
  /** Keep the schedule when the app terminates. Android only. */
  stopOnTerminate?: boolean;
  /** Start the schedule at boot. Android only. */
  startOnBoot?: boolean;
  /** Run the handler when the app is not running. Android only. */
  enableHeadless?: boolean;
  /** Schedule with AlarmManager, not JobScheduler. Android only. */
  forceAlarmManager?: boolean;
  /** The network the fetch requires. */
  requiredNetworkType?: NetworkType;
  /** Refuse to run when the battery is low. */
  requiresBatteryNotLow?: boolean;
  /** Refuse to run when storage is low. */
  requiresStorageNotLow?: boolean;
  /** Refuse to run unless the device is charging. */
  requiresCharging?: boolean;
  /** Refuse to run unless the device is idle. */
  requiresDeviceIdle?: boolean;
}

/**
 * A task scheduled by name. The Android-only fields carry `FetchConfig`'s meaning and
 * defaults.
 */
export interface TaskConfig {
  /** The id the task's own event carries. */
  taskId: string;
  /** How long to wait before the first firing, in milliseconds. */
  delayMs: number;
  /** Fire on every `delayMs`, not once. */
  periodic?: boolean;
  /** The iOS task budget the task claims. */
  taskType?: TaskType;
  /** The task needs a network connection. */
  requiresNetworkConnectivity?: boolean;
  /** Keep the schedule when the app terminates. Android only. */
  stopOnTerminate?: boolean;
  /** Start the schedule at boot. Android only. */
  startOnBoot?: boolean;
  /** Run the handler when the app is not running. Android only. */
  enableHeadless?: boolean;
  /** Schedule with AlarmManager, not JobScheduler. Android only. */
  forceAlarmManager?: boolean;
  /** The network the task requires. */
  requiredNetworkType?: NetworkType;
  /** Refuse to run when the battery is low. */
  requiresBatteryNotLow?: boolean;
  /** Refuse to run when storage is low. */
  requiresStorageNotLow?: boolean;
  /** Refuse to run unless the device is charging. */
  requiresCharging?: boolean;
  /** Refuse to run unless the device is idle. */
  requiresDeviceIdle?: boolean;
}

/** The conditions a piece of work waits for. All off: the work runs as soon as it is scheduled. */
export interface Constraints {
  /** The network the work requires, when it requires one. */
  requiredNetworkType?: NetworkType;
  /** The battery must not be low. */
  requiresBatteryNotLow?: boolean;
  /** The device must be charging. */
  requiresCharging?: boolean;
  /** The device must be idle. */
  requiresDeviceIdle?: boolean;
  /** Storage must not be low. */
  requiresStorageNotLow?: boolean;
}

/** What a piece of work looks like from the outside. */
export interface WorkInfo {
  /** The unique name the work was registered under. */
  uniqueName: string;
  /** Where the work is. */
  state: WorkState;
  /** Whether it fires on a period. */
  isPeriodic: boolean;
  /** The task the work's event names, when it has one. */
  taskName?: string;
  /** The tags the work was registered with. */
  tags: string[];
}

/** The options of one piece of one-off work. Unset fields keep the defaults. */
export interface OneOffOptions {
  /** The work's input data. Recorded here; the native module carries none in v0.1 (docs/parity.md). */
  input?: Input;
  /** How long to wait before the work runs. Undefined runs at once. */
  initialDelayMs?: number;
  /** The conditions the work waits for. */
  constraints?: Constraints;
  /** What a registration under a taken name does to the existing work. Undefined keeps it. */
  existingWorkPolicy?: ExistingWorkPolicy;
  /** How a failed attempt waits before its next one. */
  backoffPolicy?: BackoffPolicy;
  /** The first backoff delay, in milliseconds. */
  backoffPolicyDelayMs?: number;
  /** A name to cancel the work by, besides its unique name. */
  tag?: string;
}

/** The options of one piece of periodic work. Unset fields keep the defaults. */
export interface PeriodicOptions {
  /** The work's input data. Recorded here; the native module carries none in v0.1 (docs/parity.md). */
  input?: Input;
  /** The period the work fires on, in milliseconds. 15 minutes by default. */
  frequencyMs?: number;
  /** How far into a period the work may still fire, in milliseconds. */
  flexIntervalMs?: number;
  /** How long to wait before the first firing, in milliseconds. Undefined runs at once. */
  initialDelayMs?: number;
  /** The conditions the work waits for. */
  constraints?: Constraints;
  /** What a registration under a taken name does to the existing periodic work. */
  existingPeriodicWorkPolicy?: ExistingPeriodicWorkPolicy;
  /** How a failed attempt waits before its next one. */
  backoffPolicy?: BackoffPolicy;
  /** The first backoff delay, in milliseconds. */
  backoffPolicyDelayMs?: number;
  /** A name to cancel the work by, besides its unique name. */
  tag?: string;
}

/** One event the configured handler receives. */
export interface TaskEvent {
  /** The id the event carries: the fetch task id, a scheduled task's own, or a piece of work's task name. */
  taskId: string;
  /** Whether the platform gave up on the task's window. */
  timedOut: boolean;
  /** The work's input data. Fetch and scheduled-task events carry none. */
  input: Input;
}

/** What a cancel targets: one unique name, one tag, or everything. */
export type CancelBy = { uniqueName: string } | { tag: string } | "all";

/** A piece of work to register, as `Workmanager` builds it. */
export interface WorkRequest {
  /** Whether the work fires once or on a period. */
  kind: "oneOff" | "periodic";
  /** The name the work is known by. */
  uniqueName: string;
  /** The task the work's event names. */
  taskName: string;
  /** The work's input data. */
  input: Input;
  /** How long to wait before the first firing, in milliseconds. Undefined runs at once. */
  initialDelayMs?: number;
  /** The period of periodic work, in milliseconds. Undefined is the 15-minute default. */
  frequencyMs?: number;
  /** How far into a period the work may still fire, in milliseconds. */
  flexIntervalMs?: number;
  /** The conditions the work waits for. */
  constraints?: Constraints;
  /** What a registration under a taken name does to the existing one-off work. */
  existingWorkPolicy?: ExistingWorkPolicy;
  /** What a registration under a taken name does to the existing periodic work. */
  existingPeriodicWorkPolicy?: ExistingPeriodicWorkPolicy;
  /** How a failed attempt waits before its next one. */
  backoffPolicy?: BackoffPolicy;
  /** The first backoff delay, in milliseconds. */
  backoffPolicyDelayMs?: number;
  /** A name to cancel the work by, besides its unique name. */
  tag?: string;
}
