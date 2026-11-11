// The work surface driven through memoryBackend, mirroring the Rust rows in docs/goals.md.
import { expect, test } from "nsh/test";
import {
  memoryBackend,
  Workmanager,
  type BackgroundBackend,
  type CancelBy,
  type WorkInfo,
} from "../src";

test("registerOneOffTask and registerPeriodicTask validate names before the backend is touched", async () => {
  const backend = memoryBackend();
  const work = new Workmanager({ backend });

  await expect(work.registerOneOffTask("", "task")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(work.registerOneOffTask("name", "  ")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(work.registerPeriodicTask(" \t", "task")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  expect(backend.registers()).toBe(0);
  expect(backend.lastRequest()).toBeUndefined();
});

test("registerOneOffTask builds the WorkRequest the backend receives", async () => {
  const backend = memoryBackend();
  const work = new Workmanager({ backend });

  await work.registerOneOffTask("sync", "syncTask", {
    input: { attempts: 3, label: "nightly" },
    initialDelayMs: 5000,
    constraints: { requiredNetworkType: "unmetered", requiresCharging: true },
    existingWorkPolicy: "replace",
    backoffPolicy: "exponential",
    backoffPolicyDelayMs: 30000,
    tag: "media",
  });
  expect(backend.registers()).toBe(1);
  expect(backend.lastRequest()).toEqual({
    kind: "oneOff",
    uniqueName: "sync",
    taskName: "syncTask",
    input: { attempts: 3, label: "nightly" },
    initialDelayMs: 5000,
    constraints: { requiredNetworkType: "unmetered", requiresCharging: true },
    existingWorkPolicy: "replace",
    backoffPolicy: "exponential",
    backoffPolicyDelayMs: 30000,
    tag: "media",
  });

  // Without options the request carries the defaults: no delay, no constraints, empty input.
  await work.registerOneOffTask("once", "onceTask");
  expect(backend.lastRequest()).toEqual({
    kind: "oneOff",
    uniqueName: "once",
    taskName: "onceTask",
    input: {},
  });
});

test("registerPeriodicTask builds the WorkRequest the backend receives", async () => {
  const backend = memoryBackend();
  const work = new Workmanager({ backend });

  await work.registerPeriodicTask("cleaner", "cleanTask", {
    input: { depth: 2 },
    frequencyMs: 1800000,
    flexIntervalMs: 300000,
    initialDelayMs: 1000,
    constraints: { requiresBatteryNotLow: true },
    existingPeriodicWorkPolicy: "update",
    backoffPolicy: "linear",
    backoffPolicyDelayMs: 10000,
    tag: "cleanup",
  });
  expect(backend.registers()).toBe(1);
  expect(backend.lastRequest()).toEqual({
    kind: "periodic",
    uniqueName: "cleaner",
    taskName: "cleanTask",
    input: { depth: 2 },
    frequencyMs: 1800000,
    flexIntervalMs: 300000,
    initialDelayMs: 1000,
    constraints: { requiresBatteryNotLow: true },
    existingPeriodicWorkPolicy: "update",
    backoffPolicy: "linear",
    backoffPolicyDelayMs: 10000,
    tag: "cleanup",
  });
});

test("cancelByUniqueName, cancelByTag and cancelAll pass the right CancelBy", async () => {
  const backend = memoryBackend();
  // memoryBackend's cancel records only its count; this wrapper captures the CancelBy it gets.
  const bys: CancelBy[] = [];
  const recording: BackgroundBackend = {
    ...backend,
    cancel: async (by) => {
      bys.push(by);
      await backend.cancel(by);
    },
  };
  const work = new Workmanager({ backend: recording });

  await work.cancelByUniqueName("sync");
  await work.cancelByTag("media");
  await work.cancelAll();
  expect(bys).toEqual([{ uniqueName: "sync" }, { tag: "media" }, "all"]);
  expect(backend.cancels()).toBe(3);

  await expect(work.cancelByUniqueName(" ")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(work.cancelByTag("")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  expect(bys).toHaveLength(3);
  expect(backend.cancels()).toBe(3);
});

test("isScheduledByUniqueName and getWorkInfo answer from the backend", async () => {
  const backend = memoryBackend();
  const work = new Workmanager({ backend });
  const info: WorkInfo = {
    uniqueName: "sync",
    state: "running",
    isPeriodic: false,
    taskName: "syncTask",
    tags: ["media"],
  };

  backend.setIsScheduled(true);
  await expect(work.isScheduledByUniqueName("sync")).resolves.toBe(true);
  backend.setIsScheduled(false);
  await expect(work.isScheduledByUniqueName("sync")).resolves.toBe(false);

  backend.setWorkInfo(info);
  await expect(work.getWorkInfo("sync")).resolves.toEqual(info);
  backend.setWorkInfo(null);
  await expect(work.getWorkInfo("sync")).resolves.toBe(null);

  await expect(work.isScheduledByUniqueName("")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
  await expect(work.getWorkInfo("  ")).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
});
