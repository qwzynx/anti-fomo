/**
 * What the Roles page is filtered and sorted by, and the searches saved from it.
 *
 * This was page-local `$state`, so leaving Roles to read a posting on Today and
 * coming back threw away every chip. It lives here now — outside the page's
 * lifetime — and is written through to the settings table, so it survives a
 * restart too. The keys are `ui.`-prefixed: Rust does not re-rank for them.
 */

import * as api from "./api";
import type { FreshnessLabel, HubSort, Modality, TypeFilter } from "./filters";

export type DeadlineFilter = "Any" | "Has deadline" | "Closing this week";

export interface RolesFilters {
  query: string;
  type: TypeFilter;
  discipline: string;
  sources: string[];
  specialties: string[];
  modality: Modality;
  locations: string[];
  freshness: FreshnessLabel;
  seniority: string[];
  /** Annual-equivalent floor, in the posting's own currency. */
  minPay: number | null;
  /** Only postings that state a figure. */
  payDisclosed: boolean;
  deadline: DeadlineFilter;
  goodMatchOnly: boolean;
  sort: HubSort;
}

export interface SavedSearch {
  id: string;
  name: string;
  filters: RolesFilters;
  /** When it was last opened, ISO. Anything posted after this is "new". */
  viewedAt: string;
}

export const DEFAULT_FILTERS: RolesFilters = {
  query: "",
  type: "All",
  discipline: "All",
  sources: [],
  specialties: [],
  modality: "All",
  locations: [],
  freshness: "Any time",
  seniority: [],
  minPay: null,
  payDisclosed: false,
  deadline: "Any",
  goodMatchOnly: false,
  sort: "Best match",
};

const FILTERS_KEY = "ui.roles_filters";
const SAVED_KEY = "ui.saved_searches";
/** Writes coalesce: a burst of chip taps is one row update. */
const PERSIST_DELAY_MS = 400;

/** Never reassigned — always edited in place, so a page may hold a reference to it. */
const filters = $state<RolesFilters>({ ...structuredClone(DEFAULT_FILTERS) });
let saved = $state.raw<SavedSearch[]>([]);
let loaded = false;
/** The stored filters have been read. Writing before then would clobber them with defaults. */
let ready = $state(false);
let persistTimer: ReturnType<typeof setTimeout> | undefined;

/** Reads a stored blob over the defaults, dropping anything of the wrong shape. */
function revive(raw: string | null): RolesFilters {
  if (!raw) return { ...structuredClone(DEFAULT_FILTERS) };
  try {
    const parsed = JSON.parse(raw) as Partial<RolesFilters>;
    const out = { ...structuredClone(DEFAULT_FILTERS) };
    for (const key of Object.keys(DEFAULT_FILTERS) as (keyof RolesFilters)[]) {
      const value = parsed[key];
      if (value === undefined) continue;
      const fallback = DEFAULT_FILTERS[key];
      const ok =
        Array.isArray(fallback)
          ? Array.isArray(value)
          : fallback === null
            ? value === null || typeof value === "number"
            : typeof value === typeof fallback;
      if (ok) (out as Record<string, unknown>)[key] = value;
    }
    return out;
  } catch {
    return { ...structuredClone(DEFAULT_FILTERS) };
  }
}

function reviveSaved(raw: string | null): SavedSearch[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter(
        (s): s is SavedSearch =>
          !!s && typeof s.id === "string" && typeof s.name === "string" && typeof s.filters === "object",
      )
      .map((s) => ({
        ...s,
        filters: revive(JSON.stringify(s.filters)),
        viewedAt: typeof s.viewedAt === "string" ? s.viewedAt : new Date(0).toISOString(),
      }));
  } catch {
    return [];
  }
}

function persist() {
  if (!ready) return;
  clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    // Best effort: a filter that did not stick costs nothing worth a toast.
    void api.setSetting(FILTERS_KEY, JSON.stringify($state.snapshot(filters))).catch(() => {});
  }, PERSIST_DELAY_MS);
}

async function writeSaved(next: SavedSearch[]) {
  saved = next;
  await api.setSetting(SAVED_KEY, JSON.stringify(next));
}

/** Two filter sets ask the same question (the query is compared trimmed). */
export function sameFilters(a: RolesFilters, b: RolesFilters): boolean {
  return (
    JSON.stringify({ ...a, query: a.query.trim() }) ===
    JSON.stringify({ ...b, query: b.query.trim() })
  );
}

export const roles = {
  /** Live and deep: bind straight to fields. Every change is persisted. */
  get filters() {
    return filters;
  },
  get saved() {
    return saved;
  },

  /** Loads what was stored, once per session. */
  async init() {
    if (loaded) return;
    loaded = true;
    try {
      const [rawFilters, rawSaved] = await Promise.all([
        api.getSetting(FILTERS_KEY),
        api.getSetting(SAVED_KEY),
      ]);
      // A query typed before the read landed (the palette's "See all") wins.
      const query = filters.query;
      Object.assign(filters, revive(rawFilters));
      if (query) filters.query = query;
      saved = reviveSaved(rawSaved);
    } catch {
      // Defaults are a fine place to start.
    } finally {
      ready = true;
    }
  },

  get ready() {
    return ready;
  },

  /** Call after any edit to `filters`. */
  changed() {
    persist();
  },

  set(next: Partial<RolesFilters>) {
    Object.assign(filters, next);
    persist();
  },

  /** Clears every narrowing filter. Keeps the sort, which is not a filter. */
  clear() {
    Object.assign(filters, { ...structuredClone(DEFAULT_FILTERS), sort: filters.sort });
    persist();
  },

  async saveSearch(name: string) {
    const entry: SavedSearch = {
      id: crypto.randomUUID(),
      name: name.trim() || "Saved search",
      filters: $state.snapshot(filters) as RolesFilters,
      viewedAt: new Date().toISOString(),
    };
    await writeSaved([...saved, entry]);
    return entry;
  },

  /** Applies a saved search and marks it viewed, which zeroes its "new" count. */
  async apply(id: string) {
    const entry = saved.find((s) => s.id === id);
    if (!entry) return;
    Object.assign(filters, { ...structuredClone(DEFAULT_FILTERS), ...structuredClone(entry.filters) });
    persist();
    await writeSaved(
      saved.map((s) => (s.id === id ? { ...s, viewedAt: new Date().toISOString() } : s)),
    ).catch(() => {});
  },

  async removeSearch(id: string) {
    await writeSaved(saved.filter((s) => s.id !== id));
  },

  /** The saved search the current filters are exactly, if any. */
  current(): SavedSearch | null {
    return saved.find((s) => sameFilters(s.filters, filters)) ?? null;
  },
};
