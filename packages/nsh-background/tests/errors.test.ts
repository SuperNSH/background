// The pinned error contract in docs/goals.md: Rust's codes and messages, verbatim.
import { expect, test } from "nsh/test";
import { asBackgroundError, BackgroundError, type BackgroundErrorCode } from "../src";

const MESSAGES: Record<BackgroundErrorCode, string> = {
  ERR_BACKGROUND_NOT_AVAILABLE: "Background execution is not available on this target.",
  ERR_BACKGROUND_NOT_CONFIGURED: "Configure the scheduler with a task handler before starting it.",
  ERR_BACKGROUND_INVALID_ARGUMENT: "An argument is empty or not allowed on this platform.",
  ERR_BACKGROUND_BACKEND: "The platform's background scheduler failed.",
};

test("each of the four codes carries Rust's own message", () => {
  for (const [code, message] of Object.entries(MESSAGES)) {
    const error = new BackgroundError(code as BackgroundErrorCode);
    expect(error.code).toBe(code);
    expect(error.message).toBe(message);
    expect(error.name).toBe("BackgroundError");
    expect(error instanceof BackgroundError).toBe(true);
  }
});

test("asBackgroundError keeps a known code", () => {
  const kept = asBackgroundError(new BackgroundError("ERR_BACKGROUND_NOT_CONFIGURED"));
  expect(kept.code).toBe("ERR_BACKGROUND_NOT_CONFIGURED");
  expect(kept.message).toBe(MESSAGES.ERR_BACKGROUND_NOT_CONFIGURED);

  // A host rejection that already carries a known code keeps it; the message stays Rust's.
  const carried = asBackgroundError({ code: "ERR_BACKGROUND_NOT_AVAILABLE", message: "the host said no" });
  expect(carried.code).toBe("ERR_BACKGROUND_NOT_AVAILABLE");
  expect(carried.message).toBe(MESSAGES.ERR_BACKGROUND_NOT_AVAILABLE);
});

test("asBackgroundError unwraps an ERR_NSH_HOST_ERROR prefixed with a known code", () => {
  // A host's rejection carries ERR_NSH_HOST_ERROR with `<CODE>: text` as the message.
  for (const [code, message] of Object.entries(MESSAGES)) {
    const unwrapped = asBackgroundError({ code: "ERR_NSH_HOST_ERROR", message: `${code}: whatever the host said` });
    expect(unwrapped.code).toBe(code);
    expect(unwrapped.message).toBe(message);
  }
});

test("asBackgroundError maps anything unknown to ERR_BACKGROUND_BACKEND", () => {
  const backend = (error: unknown) => {
    const mapped = asBackgroundError(error);
    expect(mapped.code).toBe("ERR_BACKGROUND_BACKEND");
    expect(mapped.message).toBe(MESSAGES.ERR_BACKGROUND_BACKEND);
  };
  backend(new Error("the scheduler exploded"));
  backend({ code: "ERR_SOMETHING_ELSE", message: "not one of ours" });
  // A host error whose message names no known code is a backend failure too.
  backend({ code: "ERR_NSH_HOST_ERROR", message: "ERR_BACKGROUND_MADE_UP: not a code" });
  backend({ code: "ERR_NSH_HOST_ERROR", message: "no code anywhere" });
  backend(null);
  backend("a bare string");
});
