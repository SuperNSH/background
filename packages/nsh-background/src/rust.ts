import {
  NativeBackgroundFetch,
  type NativeOneOffOptions,
  type NativePeriodicOptions,
  type NativeTaskConfig,
} from "nsh-background/native";
import type { BackgroundBackend } from "./backend";
import { asBackgroundError } from "./errors";
import {
  STATUS_AVAILABLE,
  STATUS_DENIED,
  type CancelBy,
  type FetchConfig,
  type Status,
  type TaskConfig,
  type TaskEvent,
  type Input,
  type WorkInfo,
  type WorkRequest,
  type WorkState,
} from "./options";

/**
 * The Rust crate's scheduler and work manager (`nsh-background/native`). The app or package
 * must list the crate under `"rust"` in `nsh.json`; importing this module without it fails
 * the bundle.
 */
export function rustBackend(): BackgroundBackend {
  return new RustBackend();
}

/** The native side's numbers, as `Status` names them. Anything else reads as restricted. */
function toStatus(value: number): Status {
  if (value === STATUS_AVAILABLE) return "available";
  if (value === STATUS_DENIED) return "denied";
  return "restricted";
}

function nativeOneOffOptions(request: WorkRequest): NativeOneOffOptions {
  // The constraints spread in: their field names are the native option's own.
  const options: NativeOneOffOptions = { ...request.constraints };
  if (request.initialDelayMs !== undefined) options.initialDelayMs = request.initialDelayMs;
  if (request.existingWorkPolicy !== undefined) options.existingWorkPolicy = request.existingWorkPolicy;
  if (request.backoffPolicy !== undefined) options.backoffPolicy = request.backoffPolicy;
  if (request.backoffPolicyDelayMs !== undefined) options.backoffPolicyDelayMs = request.backoffPolicyDelayMs;
  if (request.tag !== undefined) options.tag = request.tag;
  if (request.input !== undefined) options.input = request.input;
  return options;
}

function nativePeriodicOptions(request: WorkRequest): NativePeriodicOptions {
  const options: NativePeriodicOptions = { ...request.constraints };
  if (request.frequencyMs !== undefined) options.frequencyMs = request.frequencyMs;
  if (request.flexIntervalMs !== undefined) options.flexIntervalMs = request.flexIntervalMs;
  if (request.initialDelayMs !== undefined) options.initialDelayMs = request.initialDelayMs;
  if (request.existingPeriodicWorkPolicy !== undefined) {
    options.existingPeriodicWorkPolicy = request.existingPeriodicWorkPolicy;
  }
  if (request.backoffPolicy !== undefined) options.backoffPolicy = request.backoffPolicy;
  if (request.backoffPolicyDelayMs !== undefined) options.backoffPolicyDelayMs = request.backoffPolicyDelayMs;
  if (request.tag !== undefined) options.tag = request.tag;
  if (request.input !== undefined) options.input = request.input;
  return options;
}

class RustBackend implements BackgroundBackend {
  private readonly native = new NativeBackgroundFetch();
  private onTask: ((event: TaskEvent) => void) | null = null;
  private pumping = false;

  status(): Promise<Status> {
    return this.settle(async () => toStatus(await this.native.status()));
  }

  configure(config: FetchConfig, onTask: (event: TaskEvent) => void): Promise<Status> {
    this.onTask = onTask;
    return this.settle(async () => {
      const status = toStatus(await this.native.configure(config));
      this.pump();
      return status;
    });
  }

  start(): Promise<Status> {
    return this.settle(async () => {
      const status = toStatus(await this.native.start());
      this.pump();
      return status;
    });
  }

  stop(taskId?: string | null): Promise<boolean> {
    return this.settle(async () => {
      const stopped = await this.native.stop(taskId);
      // A stop with no task id ends the crate's waiting `nextEvent`, so the pump ends with it; the
      // next `start` or `configure` runs one again.
      if (taskId === undefined || taskId === null) this.pumping = false;
      return stopped;
    });
  }

  scheduleTask(config: TaskConfig): Promise<boolean> {
    // `periodic` is the one field the native struct wants outright; the rest pass by name.
    const native: NativeTaskConfig = { ...config, periodic: config.periodic ?? false };
    return this.settle(() => this.native.scheduleTask(native));
  }

  finish(taskId: string): Promise<void> {
    return this.settle(() => this.native.finish(taskId));
  }

  register(request: WorkRequest): Promise<void> {
    if (request.kind === "oneOff") {
      return this.settle(() =>
        this.native.registerOneOffTask(request.uniqueName, request.taskName, nativeOneOffOptions(request)),
      );
    }
    return this.settle(() =>
      this.native.registerPeriodicTask(request.uniqueName, request.taskName, nativePeriodicOptions(request)),
    );
  }

  isScheduled(uniqueName: string): Promise<boolean> {
    return this.settle(() => this.native.isScheduledByUniqueName(uniqueName));
  }

  async workInfo(uniqueName: string): Promise<WorkInfo | null> {
    const info = await this.settle(() => this.native.getWorkInfo(uniqueName));
    return info === null
      ? null
      : {
          uniqueName: info.uniqueName,
          // The native side names exactly WorkState's five variants.
          state: info.state as WorkState,
          isPeriodic: info.isPeriodic,
          taskName: info.taskName ?? undefined,
          tags: [...info.tags],
        };
  }

  cancel(by: CancelBy): Promise<void> {
    if (by === "all") return this.settle(() => this.native.cancelAll());
    if ("uniqueName" in by) return this.settle(() => this.native.cancelByUniqueName(by.uniqueName));
    return this.settle(() => this.native.cancelByTag(by.tag));
  }

  // Pulls the crate's events, one `nextEvent` at a time, until a stop resolves it with null.
  private pump(): void {
    if (this.pumping) return;
    this.pumping = true;
    void (async () => {
      try {
        for (;;) {
          const event = await this.native.nextEvent();
          if (event === null) return;
          this.onTask?.({ taskId: event.taskId, timedOut: event.timedOut, input: event.input as Input });
        }
      } catch {
        // The native object went away: the stream simply ends.
        this.pumping = false;
      }
    })();
  }

  private async settle<T>(operation: () => Promise<T>): Promise<T> {
    try {
      return await operation();
    } catch (error: unknown) {
      throw asBackgroundError(error);
    }
  }
}
