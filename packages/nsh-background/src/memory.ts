import type { BackgroundBackend } from "./backend";
import { BackgroundError, type BackgroundErrorCode } from "./errors";
import type { FetchConfig, Status, TaskEvent, WorkInfo, WorkRequest } from "./options";

/** What a memory backend answers before a test scripts it. */
export interface MemoryBackendConfig {
  status?: Status;
  stopAnswer?: boolean;
  scheduleAnswer?: boolean;
  isScheduled?: boolean;
  workInfo?: WorkInfo | null;
}

/** A backend the test drives: it scripts answers, emits events and counts calls. */
export interface MemoryBackend extends BackgroundBackend {
  setStatus(status: Status): void;
  /** Makes the next `configure` reject with this code, then clears the script. */
  setConfigureError(code: BackgroundErrorCode): void;
  /** Makes the next `start` reject with this code, then clears the script. */
  setStartError(code: BackgroundErrorCode): void;
  setStopAnswer(answer: boolean): void;
  setScheduleAnswer(answer: boolean): void;
  /** Makes the next `register` reject with this code, then clears the script. */
  setRegisterError(code: BackgroundErrorCode): void;
  /** Makes the next `cancel` reject with this code, then clears the script. */
  setCancelError(code: BackgroundErrorCode): void;
  setIsScheduled(answer: boolean): void;
  setWorkInfo(info: WorkInfo | null): void;
  configures(): number;
  starts(): number;
  stops(): number;
  schedules(): number;
  finishes(): number;
  registers(): number;
  cancels(): number;
  /** Emits an event to the configured handler, as a platform timer would. */
  emit(event: TaskEvent): void;
  hasHandler(): boolean;
  lastConfig(): FetchConfig | undefined;
  lastRequest(): WorkRequest | undefined;
}

/** A memory backend, for tests and for apps that run without a scheduler. */
export function memoryBackend(config: MemoryBackendConfig = {}): MemoryBackend {
  let status = config.status ?? "available";
  let configureCode: BackgroundErrorCode | undefined;
  let startCode: BackgroundErrorCode | undefined;
  let stopAnswer = config.stopAnswer ?? false;
  let scheduleAnswer = config.scheduleAnswer ?? false;
  let registerCode: BackgroundErrorCode | undefined;
  let cancelCode: BackgroundErrorCode | undefined;
  let isScheduled = config.isScheduled ?? false;
  let workInfo = config.workInfo ?? null;
  let handler: ((event: TaskEvent) => void) | null = null;
  let lastFetchConfig: FetchConfig | undefined;
  let lastWorkRequest: WorkRequest | undefined;
  let configures = 0;
  let starts = 0;
  let stops = 0;
  let schedules = 0;
  let finishes = 0;
  let registers = 0;
  let cancels = 0;

  return {
    status: async () => status,
    configure: async (nextConfig: FetchConfig, onTask: (event: TaskEvent) => void) => {
      configures += 1;
      lastFetchConfig = nextConfig;
      handler = onTask;
      if (configureCode !== undefined) {
        const code = configureCode;
        configureCode = undefined;
        throw new BackgroundError(code);
      }
      return status;
    },
    start: async () => {
      starts += 1;
      if (startCode !== undefined) {
        const code = startCode;
        startCode = undefined;
        throw new BackgroundError(code);
      }
      return status;
    },
    stop: async () => {
      stops += 1;
      return stopAnswer;
    },
    scheduleTask: async () => {
      schedules += 1;
      return scheduleAnswer;
    },
    finish: async () => {
      finishes += 1;
    },
    register: async (request: WorkRequest) => {
      registers += 1;
      lastWorkRequest = request;
      if (registerCode !== undefined) {
        const code = registerCode;
        registerCode = undefined;
        throw new BackgroundError(code);
      }
    },
    isScheduled: async () => isScheduled,
    workInfo: async () => workInfo,
    cancel: async () => {
      // The crate's FakeBackend counts the call even when the scripted error fires.
      cancels += 1;
      if (cancelCode !== undefined) {
        const code = cancelCode;
        cancelCode = undefined;
        throw new BackgroundError(code);
      }
    },
    setStatus: (next) => {
      status = next;
    },
    setConfigureError: (code) => {
      configureCode = code;
    },
    setStartError: (code) => {
      startCode = code;
    },
    setStopAnswer: (answer) => {
      stopAnswer = answer;
    },
    setScheduleAnswer: (answer) => {
      scheduleAnswer = answer;
    },
    setRegisterError: (code) => {
      registerCode = code;
    },
    setCancelError: (code) => {
      cancelCode = code;
    },
    setIsScheduled: (answer) => {
      isScheduled = answer;
    },
    setWorkInfo: (info) => {
      workInfo = info;
    },
    configures: () => configures,
    starts: () => starts,
    stops: () => stops,
    schedules: () => schedules,
    finishes: () => finishes,
    registers: () => registers,
    cancels: () => cancels,
    emit: (event) => {
      handler?.(event);
    },
    hasHandler: () => handler !== null,
    lastConfig: () => lastFetchConfig,
    lastRequest: () => lastWorkRequest,
  };
}
