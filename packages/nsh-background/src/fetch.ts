import type { BackgroundBackend } from "./backend";
import { BackgroundError } from "./errors";
import {
  FETCH_TASK_ID,
  NETWORK_TYPE_ANY,
  NETWORK_TYPE_CELLULAR,
  NETWORK_TYPE_NONE,
  NETWORK_TYPE_NOT_ROAMING,
  NETWORK_TYPE_UNMETERED,
  STATUS_AVAILABLE,
  STATUS_DENIED,
  STATUS_RESTRICTED,
  type FetchConfig,
  type Status,
  type TaskConfig,
  type TaskEvent,
} from "./options";
import { rustBackend } from "./rust";

/**
 * The periodic-fetch scheduler, as `background_fetch` names it. An instance, where the
 * references expose statics (see `docs/parity.md`); it runs on the Rust crate unless it is
 * given another backend.
 */
export class BackgroundFetch {
  /** The references' own status numbers. */
  static readonly STATUS_RESTRICTED = STATUS_RESTRICTED;
  static readonly STATUS_DENIED = STATUS_DENIED;
  static readonly STATUS_AVAILABLE = STATUS_AVAILABLE;
  /** The references' own network-type numbers. */
  static readonly NETWORK_TYPE_NONE = NETWORK_TYPE_NONE;
  static readonly NETWORK_TYPE_ANY = NETWORK_TYPE_ANY;
  static readonly NETWORK_TYPE_UNMETERED = NETWORK_TYPE_UNMETERED;
  static readonly NETWORK_TYPE_NOT_ROAMING = NETWORK_TYPE_NOT_ROAMING;
  static readonly NETWORK_TYPE_CELLULAR = NETWORK_TYPE_CELLULAR;
  /** The task id each fetch event carries. */
  static readonly FETCH_TASK_ID = FETCH_TASK_ID;

  private readonly backend: BackgroundBackend;
  private configured = false;
  private onEvent: ((taskId: string) => void) | null = null;
  private onTimeout: ((taskId: string) => void) | null = null;

  constructor(options: { backend?: BackgroundBackend } = {}) {
    this.backend = options.backend ?? rustBackend();
  }

  /**
   * Stores the fetch settings and the pair every event routes to: `onEvent` in a task's own
   * window, `onTimeout` when the platform gives up on it. Without `onTimeout`, a timed-out
   * task is finished, the reference's own default. A zero or negative
   * `minimumFetchIntervalMinutes` rejects with `ERR_BACKGROUND_INVALID_ARGUMENT`.
   */
  configure(
    config: FetchConfig,
    onEvent: (taskId: string) => void,
    onTimeout?: (taskId: string) => void,
  ): Promise<Status> {
    const minutes = config.minimumFetchIntervalMinutes;
    if (minutes !== undefined && !(minutes > 0)) {
      return Promise.reject(new BackgroundError("ERR_BACKGROUND_INVALID_ARGUMENT"));
    }
    this.onEvent = onEvent;
    this.onTimeout =
      onTimeout ??
      ((taskId: string) => {
        console.warn(`[BackgroundFetch] default onTimeout fired for ${taskId}; finishing the task`);
        void this.finish(taskId);
      });
    return this.backend.configure(config, this.dispatch).then((status) => {
      // A configure that rejected leaves the scheduler unconfigured, as the Rust one's does.
      this.configured = true;
      return status;
    });
  }

  /** Arms the periodic fetch timer. Requires `configure` first. */
  start(): Promise<Status> {
    if (!this.configured) {
      return Promise.reject(new BackgroundError("ERR_BACKGROUND_NOT_CONFIGURED"));
    }
    return this.backend.start();
  }

  /** Stops the fetch and every timer, or just the task `taskId` names. An empty id is refused. */
  stop(taskId?: string | null): Promise<boolean> {
    if (typeof taskId === "string" && taskId.trim() === "") {
      return Promise.reject(new BackgroundError("ERR_BACKGROUND_INVALID_ARGUMENT"));
    }
    return this.backend.stop(taskId);
  }

  /** Whether background work runs here. */
  status(): Promise<Status> {
    return this.backend.status();
  }

  /** Arms a task's own timer, once or on a period. Requires configure first; a blank `taskId` is refused. */
  scheduleTask(config: TaskConfig): Promise<boolean> {
    if (!this.configured) {
      return Promise.reject(new BackgroundError("ERR_BACKGROUND_NOT_CONFIGURED"));
    }
    if (config.taskId.trim() === "") {
      return Promise.reject(new BackgroundError("ERR_BACKGROUND_INVALID_ARGUMENT"));
    }
    return this.backend.scheduleTask(config);
  }

  /** Records that the handler is done with a task. An unknown id is a no-op. */
  finish(taskId: string): Promise<void> {
    return this.backend.finish(taskId);
  }

  /** Routes one backend event to the pair the last `configure` stored. */
  private readonly dispatch = (event: TaskEvent): void => {
    if (event.timedOut) this.onTimeout?.(event.taskId);
    else this.onEvent?.(event.taskId);
  };
}
