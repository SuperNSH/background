import type { CancelBy, FetchConfig, Status, TaskConfig, TaskEvent, WorkInfo, WorkRequest } from "./options";

/** What a `BackgroundFetch` and a `Workmanager` run on: the Rust crate, or a memory backend in tests. */
export interface BackgroundBackend {
  status(): Promise<Status>;
  /** Stores the fetch settings and the handler every event goes to. */
  configure(config: FetchConfig, onTask: (event: TaskEvent) => void): Promise<Status>;
  start(): Promise<Status>;
  /** Stops the fetch and every timer, or just the task `taskId` names. */
  stop(taskId?: string | null): Promise<boolean>;
  scheduleTask(config: TaskConfig): Promise<boolean>;
  /** Records that the handler is done with a task. An unknown id is a no-op. */
  finish(taskId: string): Promise<void>;
  register(request: WorkRequest): Promise<void>;
  /** Whether the work under this unique name is scheduled or running. */
  isScheduled(uniqueName: string): Promise<boolean>;
  /** The work under this unique name, when it exists. */
  workInfo(uniqueName: string): Promise<WorkInfo | null>;
  cancel(by: CancelBy): Promise<void>;
}
