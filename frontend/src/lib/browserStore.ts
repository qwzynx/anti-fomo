"use client";

import { useSyncExternalStore } from "react";

/**
 * Reads a localStorage key as a React-subscribable external store.
 *
 * localStorage is exactly the "external system" useSyncExternalStore exists
 * for. Reading it in an effect and calling setState works but trips React 19's
 * set-state-in-effect rule and costs an extra render; this hook renders the
 * server value first, then swaps to the stored value during hydration.
 *
 * The browser's own `storage` event only fires in *other* tabs, so writers in
 * this tab must call `notifyStoreChange()` after writing.
 */

const listeners = new Set<() => void>();

export function notifyStoreChange() {
  listeners.forEach((l) => l());
}

function subscribe(callback: () => void) {
  listeners.add(callback);
  window.addEventListener("storage", callback);
  return () => {
    listeners.delete(callback);
    window.removeEventListener("storage", callback);
  };
}

export function useStoredValue(key: string, serverValue: string): string {
  return useSyncExternalStore(
    subscribe,
    () => {
      try {
        return localStorage.getItem(key) ?? serverValue;
      } catch {
        return serverValue;
      }
    },
    () => serverValue
  );
}
