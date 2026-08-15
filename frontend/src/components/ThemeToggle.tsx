"use client";

import { notifyStoreChange, useStoredValue } from "../lib/browserStore";
import { MonitorIcon, MoonIcon, SunIcon } from "./icons";

export type Theme = "light" | "dark" | "system";

export const THEME_KEY = "antifomo_theme";

/** Writes the choice to <html data-theme>; "system" removes it so the CSS
 *  media query takes over. Kept in sync with the boot script in layout.tsx. */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

const OPTIONS: { value: Theme; label: string; Icon: typeof SunIcon }[] = [
  { value: "light", label: "Light", Icon: SunIcon },
  { value: "system", label: "System", Icon: MonitorIcon },
  { value: "dark", label: "Dark", Icon: MoonIcon },
];

export default function ThemeToggle() {
  // Server renders "system"; hydration swaps in the stored choice.
  const theme = useStoredValue(THEME_KEY, "system") as Theme;

  function choose(next: Theme) {
    localStorage.setItem(THEME_KEY, next);
    applyTheme(next);
    notifyStoreChange();
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
