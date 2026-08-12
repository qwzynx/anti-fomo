export type Theme = "light" | "dark" | "system";

const KEY = "antifomo_theme";

function initial(): Theme {
  if (typeof localStorage === "undefined") return "system";
  const saved = localStorage.getItem(KEY);
  return saved === "light" || saved === "dark" ? saved : "system";
}

// The boot script in app.html applies the class before first paint; this store
// keeps it in sync afterwards.
let current = $state<Theme>(initial());

function apply(theme: Theme) {
  const dark =
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
}

export const theme = {
  get value() {
    return current;
  },
  set(next: Theme) {
    current = next;
    localStorage.setItem(KEY, next);
    apply(next);
  },
  /** Cycles light → dark → system, which is how the header button behaves. */
  cycle() {
    this.set(current === "light" ? "dark" : current === "dark" ? "system" : "light");
  },
  /** Keeps "system" honest when the OS preference changes while running. */
  watchSystem() {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => current === "system" && apply("system");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  },
};
