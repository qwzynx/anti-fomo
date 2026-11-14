/**
 * Short-lived notices: "Saved", "Hidden — Undo", "Could not save".
 *
 * The app used to have one place to say anything had gone wrong — the banner
 * under the top bar — and it always read "last refresh failed", so a save that
 * did not stick was reported as a scrape that had not run. The banner is for
 * the state of the cache now; anything that is about a single action the reader
 * just took is a toast, next to the moment it happened, and gone a few seconds
 * later.
 */

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
  /** One button, e.g. Undo. Running it dismisses the toast. */
  action?: { label: string; run: () => void | Promise<void> };
}

/** Long enough to read and reach for Undo; errors stay a little longer. */
const DURATION_MS: Record<ToastKind, number> = { info: 5000, success: 4000, error: 8000 };
/** More than this on screen at once is noise; the oldest goes first. */
const MAX_VISIBLE = 3;

let toasts = $state<Toast[]>([]);
let nextId = 1;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

function dismiss(id: number) {
  const timer = timers.get(id);
  if (timer) clearTimeout(timer);
  timers.delete(id);
  toasts = toasts.filter((t) => t.id !== id);
}

function push(kind: ToastKind, message: string, action?: Toast["action"]): number {
  const id = nextId++;
  const next = [...toasts, { id, kind, message, action }];
  while (next.length > MAX_VISIBLE) {
    const dropped = next.shift();
    if (dropped) {
      const timer = timers.get(dropped.id);
      if (timer) clearTimeout(timer);
      timers.delete(dropped.id);
    }
  }
  toasts = next;
  timers.set(
    id,
    setTimeout(() => dismiss(id), DURATION_MS[kind]),
  );
  return id;
}

/** Turns whatever a rejected `invoke` threw into one readable line. */
export function describeError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

export const toast = {
  get all() {
    return toasts;
  },
  info: (message: string, action?: Toast["action"]) => push("info", message, action),
  success: (message: string, action?: Toast["action"]) => push("success", message, action),
  /** `what` reads as the end of "Could not …", e.g. "save the role". */
  error: (what: string, e?: unknown) =>
    push("error", e === undefined ? `Could not ${what}.` : `Could not ${what}: ${describeError(e)}`),
  dismiss,
  /** Pauses auto-dismiss while the pointer or focus is on a toast. */
  hold(id: number) {
    const timer = timers.get(id);
    if (timer) clearTimeout(timer);
    timers.delete(id);
  },
  release(id: number) {
    const t = toasts.find((x) => x.id === id);
    if (!t || timers.has(id)) return;
    timers.set(
      id,
      setTimeout(() => dismiss(id), DURATION_MS[t.kind]),
    );
  },
};
