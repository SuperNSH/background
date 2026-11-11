import type { BackgroundBackend } from "./backend";
import { BackgroundError } from "./errors";
import type { OneOffOptions, PeriodicOptions, WorkInfo, WorkRequest } from "./options";
import { rustBackend } from "./rust";

/**
 * The work manager, as `workmanager` names it. It runs on the Rust crate unless it is given
 * another backend. Work needs no configure; its events wait for a configured fetch handler.
 */
export class Workmanager {
  private readonly backend: BackgroundBackend;

  constructor(options: { backend?: BackgroundBackend } = {}) {
    this.backend = options.backend ?? rustBackend();
  }

  /** Registers work that fires once, `initialDelayMs` after the call at the latest. */
  async registerOneOffTask(uniqueName: string, taskName: string, options: OneOffOptions = {}): Promise<void> {
    this.checkNames(uniqueName, taskName);
    const request: WorkRequest = {
      kind: "oneOff",
      uniqueName,
      taskName,
      input: options.input ?? {},
      initialDelayMs: options.initialDelayMs,
      constraints: options.constraints,
      existingWorkPolicy: options.existingWorkPolicy,
      backoffPolicy: options.backoffPolicy,
      backoffPolicyDelayMs: options.backoffPolicyDelayMs,
      tag: options.tag,
    };
    await this.backend.register(request);
  }

  /** Registers work that fires on every `frequencyMs`, the 15-minute default when unset. */
  async registerPeriodicTask(uniqueName: string, taskName: string, options: PeriodicOptions = {}): Promise<void> {
    this.checkNames(uniqueName, taskName);
    const request: WorkRequest = {
      kind: "periodic",
      uniqueName,
      taskName,
      input: options.input ?? {},
      initialDelayMs: options.initialDelayMs,
      frequencyMs: options.frequencyMs,
      flexIntervalMs: options.flexIntervalMs,
      constraints: options.constraints,
      existingPeriodicWorkPolicy: options.existingPeriodicWorkPolicy,
      backoffPolicy: options.backoffPolicy,
      backoffPolicyDelayMs: options.backoffPolicyDelayMs,
      tag: options.tag,
    };
    await this.backend.register(request);
  }

  /** Cancels the work under `uniqueName`. */
  async cancelByUniqueName(uniqueName: string): Promise<void> {
    this.checkNames(uniqueName);
    await this.backend.cancel({ uniqueName });
  }

  /** Cancels every work registered with `tag`. */
  async cancelByTag(tag: string): Promise<void> {
    this.checkNames(tag);
    await this.backend.cancel({ tag });
  }

  /** Cancels every piece of work. */
  async cancelAll(): Promise<void> {
    await this.backend.cancel("all");
  }

  /** Whether the work under `uniqueName` is scheduled or running. */
  async isScheduledByUniqueName(uniqueName: string): Promise<boolean> {
    this.checkNames(uniqueName);
    return this.backend.isScheduled(uniqueName);
  }

  /** The work under `uniqueName`, when it exists. */
  async getWorkInfo(uniqueName: string): Promise<WorkInfo | null> {
    this.checkNames(uniqueName);
    return this.backend.workInfo(uniqueName);
  }

  /** Empty or whitespace names are refused before the backend sees the call. */
  private checkNames(...names: string[]): void {
    if (names.some((name) => name.trim() === "")) {
      throw new BackgroundError("ERR_BACKGROUND_INVALID_ARGUMENT");
    }
  }
}
