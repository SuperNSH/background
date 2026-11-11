/** The codes a background rejection carries. Apps match on `code`. */
export type BackgroundErrorCode =
  | "ERR_BACKGROUND_NOT_AVAILABLE"
  | "ERR_BACKGROUND_NOT_CONFIGURED"
  | "ERR_BACKGROUND_INVALID_ARGUMENT"
  | "ERR_BACKGROUND_BACKEND";

/** Same wording as the Rust crate's messages. None holds the reason text. */
const MESSAGES: Record<BackgroundErrorCode, string> = {
  ERR_BACKGROUND_NOT_AVAILABLE: "Background execution is not available on this target.",
  ERR_BACKGROUND_NOT_CONFIGURED: "Configure the scheduler with a task handler before starting it.",
  ERR_BACKGROUND_INVALID_ARGUMENT: "An argument is empty or not allowed on this platform.",
  ERR_BACKGROUND_BACKEND: "The platform's background scheduler failed.",
};

const KNOWN = new Set<string>(Object.keys(MESSAGES));

/** A rejection of the background API. `code` is the stable name; the message says what to do. */
export class BackgroundError extends Error {
  readonly code: BackgroundErrorCode;

  constructor(code: BackgroundErrorCode) {
    super(MESSAGES[code]);
    this.name = "BackgroundError";
    this.code = code;
  }
}

/**
 * Turns whatever the native side or a host threw into a coded `BackgroundError`. A host's
 * rejection carries `ERR_NSH_HOST_ERROR`, and its message starts with the code, as
 * `ERR_BACKGROUND_NOT_CONFIGURED: text`. Anything unknown is a backend failure.
 */
export function asBackgroundError(error: unknown): BackgroundError {
  if (error instanceof BackgroundError) return error;
  const raw = (error ?? {}) as { code?: unknown; message?: unknown };
  const code = typeof raw.code === "string" ? raw.code : "";
  const message = typeof raw.message === "string" ? raw.message : "";
  if (code === "ERR_NSH_HOST_ERROR") {
    const named = /^(ERR_BACKGROUND_[A-Z_]+): /.exec(message)?.[1];
    return named !== undefined && KNOWN.has(named)
      ? new BackgroundError(named as BackgroundErrorCode)
      : new BackgroundError("ERR_BACKGROUND_BACKEND");
  }
  return KNOWN.has(code)
    ? new BackgroundError(code as BackgroundErrorCode)
    : new BackgroundError("ERR_BACKGROUND_BACKEND");
}
