/**
 * One confirm dialog for the whole app, awaited like `window.confirm`.
 *
 * Settings and the résumé builder each grew their own two-step inline
 * confirm, and the skills form's "Clear all" had none. This is the shared
 * one: `if (await confirm({...})) doIt()`. The dialog itself is mounted once,
 * in the root layout.
 */

export interface ConfirmRequest {
  title: string;
  body?: string;
  /** The button that does the thing, e.g. "Delete résumé". */
  confirmLabel: string;
  cancelLabel?: string;
  /** Destructive actions get the danger colour. */
  danger?: boolean;
}

let pending = $state<(ConfirmRequest & { resolve: (ok: boolean) => void }) | null>(null);

export function confirm(request: ConfirmRequest): Promise<boolean> {
  // A second request while one is up answers the first "no".
  pending?.resolve(false);
  return new Promise((resolve) => {
    pending = { ...request, resolve };
  });
}

export const confirmState = {
  get current() {
    return pending;
  },
  answer(ok: boolean) {
    const p = pending;
    pending = null;
    p?.resolve(ok);
  },
};
