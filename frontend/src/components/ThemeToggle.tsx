"use client";

import { useSyncExternalStore } from "react";
import { MoonIcon, SunIcon } from "./icons";

export type Theme = "light" | "dark";

export const THEME_KEY = "antifomo_theme";

/** Writes the choice to <html data-theme>. Always one of the two themes —
 *  there is no "follow the OS" state. Kept in sync with the boot script in
 *  layout.tsx. */
export function applyTheme(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
}

const listeners = new Set<() => void>();

function subscribe(callback: () => void) {
  listeners.add(callback);
  window.addEventListener("storage", callback);
  return () => {
    listeners.delete(callback);
    window.removeEventListener("storage", callback);
  };
}

/** The stored choice, or the OS preference as the first-visit default. Once
 *  the user picks, their choice sticks regardless of what the OS does. */
function readTheme(): Theme {
  try {
    const stored = localStorage.getItem(THEME_KEY);
    if (stored === "light" || stored === "dark") return stored;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  } catch {
    return "dark";
  }
}

const OPTIONS: { value: Theme; label: string; Icon: typeof SunIcon }[] = [
  { value: "light", label: "Light", Icon: SunIcon },
  { value: "dark", label: "Dark", Icon: MoonIcon },
];

export default function ThemeToggle() {
  // The server cannot know the OS preference, so it renders "dark" and
  // hydration swaps in the real value. useSyncExternalStore is the sanctioned
  // way to do that without a mismatch warning.
  const theme = useSyncExternalStore(subscribe, readTheme, () => "dark" as Theme);

  function choose(next: Theme) {
    localStorage.setItem(THEME_KEY, next);
    applyTheme(next);
    listeners.forEach((l) => l());
  }

  return (
    <div
      role="group"
      aria-label="Colour theme"
      className="flex items-center gap-0.5 rounded-full border border-line bg-sunken p-0.5"
    >
      {OPTIONS.map(({ value, label, Icon }) => {
        const active = theme === value;
        return (
          <button
            key={value}
            type="button"
            onClick={() => choose(value)}
            aria-label={label}
            aria-pressed={active}
            title={label}
            className={`flex h-7 w-7 items-center justify-center rounded-full transition-colors ${
              active
                ? "bg-card text-ink shadow-[var(--shadow-card)]"
                : "text-ink-3 hover:text-ink"
            }`}
          >
            <Icon className="h-3.5 w-3.5" />
          </button>
        );
      })}
    </div>
  );
}
