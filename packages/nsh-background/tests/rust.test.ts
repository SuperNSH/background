import { expect, test } from "nsh/test";
import { BackgroundFetch, Workmanager } from "../src";
import { rustBackend } from "../src/rust";

// Through the crate itself: work with input crosses the bridge, and a stop with no task id ends
// the event pump so nothing is left waiting on the native side.
test("the crate takes work with input data and stops without leaving a read waiting", async () => {
  const backend = rustBackend();
  const fetch = new BackgroundFetch({ backend });
  const work = new Workmanager({ backend });

  await fetch.configure({}, () => {});
  await work.registerOneOffTask("rust-input", "task", {
    input: { count: 1, label: "x", on: true },
    initialDelayMs: 3_600_000,
  });
  expect(await work.isScheduledByUniqueName("rust-input")).toBe(true);

  await work.cancelAll();
  expect(typeof (await fetch.stop())).toBe("boolean");
});

test("work input that is not flat is refused by the crate", async () => {
  const work = new Workmanager({ backend: rustBackend() });
  const nested = { deep: [1] } as unknown as Record<string, number>;
  await expect(work.registerOneOffTask("rust-nested", "task", { input: nested })).rejects.toMatchObject({
    code: "ERR_BACKGROUND_INVALID_ARGUMENT",
  });
});
