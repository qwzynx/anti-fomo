import { LayoutGrid, LayoutList, Rows3, type IconComponent } from "./icons";

/**
 * How tightly the feed packs its results. With ~1,600 items cached, cards are
 * the wrong shape for scanning a long list, so the same data renders three
 * ways and the choice sticks across launches:
 *
 *   cards   — the grid: logo, summary, tags. Best for browsing.
 *   list    — one full-width row per item, summary kept.
 *   compact — one line per item. Fits ~4× as many results on screen.
 */
export type Density = "cards" | "list" | "compact";

const KEY = "antifomo_density";

export const DENSITIES: { value: Density; label: string; icon: IconComponent }[] = [
  { value: "cards", label: "Cards", icon: LayoutGrid },
  { value: "list", label: "List", icon: LayoutList },
  { value: "compact", label: "Compact", icon: Rows3 },
];

function initial(): Density {
  // The static shell is prerendered at build time, where there is no storage.
  if (typeof localStorage === "undefined") return "cards";
  const saved = localStorage.getItem(KEY);
  return saved === "list" || saved === "compact" ? saved : "cards";
}

let current = $state<Density>(initial());

export const density = {
  get value() {
    return current;
  },
  set(next: Density) {
    current = next;
    localStorage.setItem(KEY, next);
  },
};
