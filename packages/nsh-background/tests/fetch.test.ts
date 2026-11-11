// The fetch surface driven through memoryBackend, mirroring the Rust rows in docs/goals.md.
import { expect, test } from "nsh/test";
import {
  BackgroundFetch,
  FETCH_TASK_ID,
  memoryBackend,
  NETWORK_TYPE_ANY,
  NETWORK_TYPE_CELLULAR,
  NETWORK_TYPE_NONE,
  NETWORK_TYPE_NOT_ROAMING,
  NETWORK_TYPE_UNMETERED,
  STATUS_AVAILABLE,
  STATUS_DENIED,
  STATUS_RESTRICTED,
} from "../src";

test("configure stores the handler, and a re-configure replaces it", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });
  const first: string[] = [];
  const second: string[] = [];

  await fetch.configure({ minimumFetchIntervalMinutes: 30 }, (taskId) => first.push(taskId));
  expect(backend.configures()).toBe(1);
  expect(backend.hasHandler()).toBe(true);
  expect(backend.lastConfig()).toEqual({ minimumFetchIntervalMinutes: 30 });

  backend.emit({ taskId: FETCH_TASK_ID, timedOut: false, input: {} });
  expect(first).toEqual([FETCH_TASK_ID]);

  await fetch.configure({}, (taskId) => second.push(taskId));
  expect(backend.configures()).toBe(2);
  backend.emit({ taskId: FETCH_TASK_ID, timedOut: false, input: {} });
  expect(first).toEqual([FETCH_TASK_ID]);
  expect(second).toEqual([FETCH_TASK_ID]);
});

test("emit routes by timedOut to onTimeout or onEvent", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });
  const events: string[] = [];
  const timeouts: string[] = [];

  await fetch.configure({}, (taskId) => events.push(taskId), (taskId) => timeouts.push(taskId));
  backend.emit({ taskId: "task-1", timedOut: false, input: {} });
  expect(events).toEqual(["task-1"]);
  expect(timeouts).toEqual([]);

  backend.emit({ taskId: "task-1", timedOut: true, input: {} });
  expect(events).toEqual(["task-1"]);
  expect(timeouts).toEqual(["task-1"]);
});

test("the default onTimeout finishes the task", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });
  const events: string[] = [];

  await fetch.configure({}, (taskId) => events.push(taskId));
  backend.emit({ taskId: "task-1", timedOut: true, input: {} });
  // finish() runs its body synchronously; one microtask lets the voided promise settle.
  await Promise.resolve();
  expect(backend.finishes()).toBe(1);
  expect(events).toEqual([]);
});

test("start and scheduleTask before configure reject NOT_CONFIGURED, while status, stop and finish still resolve", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });

  await expect(fetch.start()).rejects.toMatchObject({ code: "ERR_BACKGROUND_NOT_CONFIGURED" });
  await expect(fetch.scheduleTask({ taskId: "task-1", delayMs: 1000 })).rejects.toMatchObject({
    code: "ERR_BACKGROUND_NOT_CONFIGURED",
  });
  expect(backend.starts()).toBe(0);
  expect(backend.schedules()).toBe(0);

  await expect(fetch.status()).resolves.toBe("available");
  await expect(fetch.stop()).resolves.toBe(false);
  await fetch.finish("task-1");
  expect(backend.stops()).toBe(1);
  expect(backend.finishes()).toBe(1);
});

test("invalid arguments reject INVALID_ARGUMENT without touching the backend", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });

  await expect(fetch.configure({ minimumFetchIntervalMinutes: 0 }, () => {})).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(fetch.configure({ minimumFetchIntervalMinutes: -5 }, () => {})).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  expect(backend.configures()).toBe(0);
  expect(backend.hasHandler()).toBe(false);

  await fetch.configure({}, () => {});
  await expect(fetch.scheduleTask({ taskId: "   ", delayMs: 1000 })).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(fetch.stop("")).rejects.toMatchObject({ code: "ERR_BACKGROUND_INVALID_ARGUMENT" });
  expect(backend.schedules()).toBe(0);
  expect(backend.stops()).toBe(0);
});

test("start, stop, status, scheduleTask and finish pass through to the backend", async () => {
  const backend = memoryBackend();
  const fetch = new BackgroundFetch({ backend });
  await fetch.configure({}, () => {});

  backend.setStatus("denied");
  await expect(fetch.start()).resolves.toBe("denied");
  expect(backend.starts()).toBe(1);

  backend.setStopAnswer(true);
  await expect(fetch.stop()).resolves.toBe(true);
  await expect(fetch.stop("task-1")).resolves.toBe(true);
  expect(backend.stops()).toBe(2);

  backend.setScheduleAnswer(true);
  await expect(fetch.scheduleTask({ taskId: "task-1", delayMs: 60000, periodic: true })).resolves.toBe(true);
  expect(backend.schedules()).toBe(1);

  await expect(fetch.status()).resolves.toBe("denied");
  await fetch.finish("task-1");
  expect(backend.finishes()).toBe(1);
});

test("the constants keep the references' values", () => {
  expect(BackgroundFetch.STATUS_RESTRICTED).toBe(0);
  expect(BackgroundFetch.STATUS_DENIED).toBe(1);
  expect(BackgroundFetch.STATUS_AVAILABLE).toBe(2);
  expect(BackgroundFetch.NETWORK_TYPE_NONE).toBe(0);
  expect(BackgroundFetch.NETWORK_TYPE_ANY).toBe(1);
  expect(BackgroundFetch.NETWORK_TYPE_UNMETERED).toBe(2);
  expect(BackgroundFetch.NETWORK_TYPE_NOT_ROAMING).toBe(3);
  expect(BackgroundFetch.NETWORK_TYPE_CELLULAR).toBe(4);
  expect(BackgroundFetch.FETCH_TASK_ID).toBe("flutter_background_fetch");

  expect(STATUS_RESTRICTED).toBe(0);
  expect(STATUS_DENIED).toBe(1);
  expect(STATUS_AVAILABLE).toBe(2);
  expect(NETWORK_TYPE_NONE).toBe(0);
  expect(NETWORK_TYPE_ANY).toBe(1);
  expect(NETWORK_TYPE_UNMETERED).toBe(2);
  expect(NETWORK_TYPE_NOT_ROAMING).toBe(3);
  expect(NETWORK_TYPE_CELLULAR).toBe(4);
  expect(FETCH_TASK_ID).toBe("flutter_background_fetch");
});
