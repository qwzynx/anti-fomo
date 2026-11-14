import { error as logError } from "@tauri-apps/plugin-log";
import { describeError, toast } from "./toast.svelte";

/**
 * The last line of defence for the UI: anything thrown or rejected that no
 * caller handled is written to the app's log file (where a bug report can
 * find it) and shown once, rather than vanishing into a devtools console no
 * user of a packaged app ever opens.
 *
 * Repeats are folded: a broken effect can throw on every frame, and a toast
 * per frame would bury the window.
 */
const QUIET_MS = 10_000;
const recent = new Map<string, number>();

function report(kind: string, reason: unknown) {
  const message = describeError(reason);
  const now = Date.now();
  const last = recent.get(message);
  if (last !== undefined && now - last < QUIET_MS) return;
  recent.set(message, now);
  if (recent.size > 50) recent.clear();

  const stack = reason instanceof Error && reason.stack ? `\n${reason.stack}` : "";
  void logError(`[ui] ${kind}: ${message}${stack}`).catch(() => {});
  toast.error("finish that", message);
}

/** Installs the handlers; returns the uninstaller. Called once from the root layout. */
export function installErrorHandlers(): () => void {
  const onError = (e: ErrorEvent) => report("uncaught error", e.error ?? e.message);
  const onRejection = (e: PromiseRejectionEvent) => report("unhandled rejection", e.reason);
  window.addEventListener("error", onError);
  window.addEventListener("unhandledrejection", onRejection);
  return () => {
    window.removeEventListener("error", onError);
    window.removeEventListener("unhandledrejection", onRejection);
  };
}
