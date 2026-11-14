import { listen } from "@tauri-apps/api/event";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
// Namespaced: several store methods below intentionally share a name with the
// command they wrap, and `api.setSaved(…)` makes it obvious which is which.
import * as api from "./api";
import type {
  Application,
  ApplicationStatus,
  FeedStatus,
  OnboardingState,
  RefreshOutcome,
  ScanPhase,
  ScrapedItem,
  ScrapePrefs,
  ScrapeProgress,
  SetupOptions,
  SkillCategory,
} from "./api";
import { MIN_SKILLS_TO_SHOW } from "./skills";
import { toast } from "./toast.svelte";

const DEFAULT_MAJOR = "Software Engineering";

/** Everything wanted, which is what a reader who has said nothing gets. */
const DEFAULT_PREFS: ScrapePrefs = {
  internships: true,
  jobs: true,
  news: true,
  events: true,
  disabled_sources: [],
};

/**
 * While a scrape is writing, how often the lists are reloaded. Every source
 * that finishes moves the data, and a reload is five commands and a re-rank, so
 * reloading per event would spend the whole scrape redrawing. A few seconds
 * keeps the feed visibly filling without that.
 */
const SCAN_RELOAD_MS = 3000;
/**
 * A load is five commands, a re-rank and — at forty thousand roles — a payload
 * of twenty-odd megabytes parsed on the window's own thread, and measured against
 * the real app that parse is the stutter. So the pace follows the cost: a reload
 * is spaced at least this many times its own duration after the last one, up to
 * a cap, and a small cache keeps the quick cadence above.
 */
const SCAN_RELOAD_COST_FACTOR = 5;
const SCAN_RELOAD_MAX_MS = 20000;
/** How far down a list the reader can be before an update would move it. */
const SCROLLED_AWAY_PX = 120;

/**
 * How long after the last skill tap the feed is re-ranked. Long enough to
 * absorb a burst of taps while reading one posting, short enough that the new
 * order is there by the time the user looks back at the list.
 */
const SKILL_RERANK_DELAY_MS = 1200;

// One shared store rather than a fetch in every page: the feed, the internships
// hub and the saved list all read the same local database, and the header's
// refresh button has to drive all of them.
//
// The three lists are `$state.raw` on purpose. `get_internships` returns the
// whole opportunity cache — 17,739 rows against the real database — and plain
// `$state` deep-proxies every one of them plus their nested arrays the moment
// they are assigned. That proxying alone froze the window for seconds on every
// load, and nothing needed it: a scrape result is a snapshot, replaced whole
// when it changes, never edited field by field.
//
// What *is* edited field by field — saved, seen, skill coverage — lives in the
// three reactive sets below instead, keyed by URL. They are small, they are the
// same answer everywhere the item appears, and they update without touching the
// lists at all.
let items = $state.raw<ScrapedItem[]>([]);
let internships = $state.raw<ScrapedItem[]>([]);
let saved = $state.raw<ScrapedItem[]>([]);
/** Strong postings closing within a week. Its own list: see `api.getLastCall`. */
let lastCall = $state.raw<ScrapedItem[]>([]);
let status = $state<FeedStatus | null>(null);
let major = $state(DEFAULT_MAJOR);
let interests = $state.raw<string[]>([]);
let availableInterests = $state.raw<string[]>([]);
let skills = $state.raw<string[]>([]);
let skillCatalog = $state.raw<SkillCategory[]>([]);

/** Live per-item state, keyed by URL, shared across every list an item is in. */
const savedUrls = new SvelteSet<string>();
const seenUrls = new SvelteSet<string>();
/** Application status and note per saved role, keyed by URL. */
const applications = new SvelteMap<string, Application>();
/** The profile as a set, so a coverage check is a lookup and not a scan. */
const skillSet = new SvelteSet<string>();

// null until the user has been through the form once. A stored flag rather
// than "is the skill list empty", because finishing the form having picked
// nothing and never having opened it must look different — otherwise the
// setup prompt never goes away.
let skillsSetupAt = $state<string | null>(null);

// --- what to scrape ---
//
// `unknown` until Rust has answered, so a returning reader never sees the form
// flash for the instant before their state arrives. `needed` blocks everything:
// nothing has been fetched and nothing will be until it is answered.
let onboarding = $state<"unknown" | OnboardingState>("unknown");
let prefs = $state.raw<ScrapePrefs>(DEFAULT_PREFS);
let setupOptions = $state.raw<SetupOptions>({ seniority: [], regions: [] });
let homeRegion = $state<string | null>(null);
let targetSeniority = $state.raw<string[]>([]);
/** The form re-opened from Settings, by someone who has long since answered it. */
let setupOpen = $state(false);
/** One entry per source that has finished in the current refresh. */
let progress = $state.raw<ScrapeProgress[]>([]);
/** Callers waiting for the refresh that is already under way to end. */
let idleWaiters: Array<() => void> = [];
let skillsFormOpen = $state(false);
let loading = $state(true);
let refreshing = $state(false);
/** Which part of a refresh is running, driven by `feed:phase`. */
let phase = $state<ScanPhase>("idle");
/** Sources the running refresh will ask, from `feed:plan`. */
let planned = $state(0);
/** Rows the running scrape has written so far. */
let scanRows = $state(0);
/** New results are waiting behind the "Show" pill because the reader is mid-list. */
let pending = $state(false);
let error = $state<string | null>(null);
let started = false;
let skillRerankTimer: ReturnType<typeof setTimeout> | null = null;
/** Coalesces the stream of `feed:updated` events a refresh emits. */
let reloadTimer: ReturnType<typeof setTimeout> | null = null;
let reloading = false;
/** An event arrived while a load was in flight, so one more is owed. */
let dirty = false;
let lastLoadAt = 0;
/** How long the last reload took, which sets how soon the next may start. */
let lastLoadMs = 0;
/** Orders loads: a slow one that finishes late must not overwrite a newer one. */
let loadSeq = 0;
let appliedSeq = 0;
/** Whether the reader has scrolled a list down, so an update would move it. */
let scrolledAway = false;

/** Postings the detail pane has already fetched, keyed by URL. */
const detailCache = new Map<string, ScrapedItem | null>();
const DETAIL_CACHE_LIMIT = 200;

/** A JSON array stored as a setting, tolerating a missing or corrupt value. */
function parseList(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

/** Resolves when no refresh is running — immediately if none is. */
function whenIdle(): Promise<void> {
  if (!refreshing) return Promise.resolve();
  return new Promise((resolve) => idleWaiters.push(resolve));
}

/** Replaces a set's contents in one pass, without notifying per element. */
function reset(set: SvelteSet<string>, values: Iterable<string>) {
  set.clear();
  for (const value of values) set.add(value);
}

async function loadAll() {
  const mine = ++loadSeq;
  const [feedItems, internshipItems, savedItems, lastCallItems, st, apps] = await Promise.all([
    api.getFeed(major),
    api.getInternships(major),
    api.getSaved(),
    api.getLastCall(major),
    api.feedStatus(),
    // The tracker is a nicety on top of the lists: a failure here must not
    // take the whole load down with it.
    api.getApplications().catch(() => null),
  ]);
  // Superseded while it was in flight: a newer load has already landed.
  if (mine < appliedSeq) return;
  appliedSeq = mine;
  pending = false;
  items = feedItems;
  internships = internshipItems;
  saved = savedItems;
  lastCall = lastCallItems;
  status = st;
  refreshing = st.refreshing;
  // A reload means a scrape or an enrichment pass landed, so a posting the
  // pane cached with no description may have one now.
  detailCache.clear();

  // Seeded from what Rust just sent, then owned by the UI until the next load.
  // `saved` is the authoritative list of stars; the feed rows only repeat it.
  reset(
    savedUrls,
    savedItems.map((i) => i.url),
  );
  const seen: string[] = [];
  for (const list of [feedItems, internshipItems, savedItems, lastCallItems]) {
    for (const item of list) if (item.seen) seen.push(item.url);
  }
  reset(seenUrls, seen);
  if (apps) {
    applications.clear();
    for (const a of apps) applications.set(a.url, a);
  }
}

/**
 * Whether a reload now would move rows under someone reading them. Only while a
 * scrape is writing, only when there is something on screen to move, and only
 * when the reader has scrolled away from the top — a list they are at the top of
 * is one they are watching fill.
 */
function shouldHold(): boolean {
  return refreshing && internships.length > 0 && scrolledAway;
}

/**
 * Reloads the lists, at most one at a time and at most every few seconds while
 * a scrape is writing.
 *
 * A refresh emits `feed:updated` per source and per group of descriptions, and
 * a settings change can land on top of any of them. Each reload is five
 * commands and a re-rank. A load already in flight is not dropped — an event
 * that arrives during it marks the store dirty and one more follows — and one
 * already scheduled is left alone, so a steady stream becomes a steady pace
 * and not a postponement.
 */
function scheduleReload(now = false) {
  dirty = true;
  if (reloading || reloadTimer) return;
  const spacing = Math.min(
    SCAN_RELOAD_MAX_MS,
    Math.max(SCAN_RELOAD_MS, lastLoadMs * SCAN_RELOAD_COST_FACTOR),
  );
  const wait = now ? 0 : refreshing ? Math.max(0, spacing - (Date.now() - lastLoadAt)) : 150;
  reloadTimer = setTimeout(runReload, wait);
}

async function runReload() {
  reloadTimer = null;
  if (reloading) return;
  if (shouldHold()) {
    // Left dirty-free: the pill, or the reader returning to the top, reloads.
    dirty = false;
    pending = true;
    return;
  }
  reloading = true;
  dirty = false;
  const started = Date.now();
  try {
    await loadAll();
  } catch (e) {
    error = String(e);
  } finally {
    reloading = false;
    lastLoadAt = Date.now();
    lastLoadMs = lastLoadAt - started;
    if (dirty) scheduleReload();
  }
}

/** Loads now, past the hold, and resolves when it has landed. */
async function reloadNow() {
  if (reloadTimer) {
    clearTimeout(reloadTimer);
    reloadTimer = null;
  }
  dirty = false;
  lastLoadAt = Date.now();
  await loadAll();
}

/** Tracks whether any list on screen has been scrolled away from its top. */
function watchScrolling() {
  if (typeof document === "undefined") return;
  document.addEventListener(
    "scroll",
    (e) => {
      const target = e.target;
      const el =
        target instanceof Element
          ? target
          : (document.scrollingElement ?? document.documentElement);
      const away = el.scrollTop > SCROLLED_AWAY_PX;
      if (away === scrolledAway) return;
      scrolledAway = away;
      // Back at the top: whatever was held can land without moving anything.
      if (!away && pending) scheduleReload(true);
    },
    { capture: true, passive: true },
  );
}

/** Re-ranks once the taps stop, so the list does not reshuffle mid-burst. */
function scheduleRerank() {
  if (skillRerankTimer) clearTimeout(skillRerankTimer);
  skillRerankTimer = setTimeout(() => {
    skillRerankTimer = null;
    scheduleReload();
  }, SKILL_RERANK_DELAY_MS);
}

export const feed = {
  get items() {
    return items;
  },
  get internships() {
    return internships;
  },
  get saved() {
    return saved;
  },
  get onboarding() {
    return onboarding;
  },
  get prefs() {
    return prefs;
  },
  get setupOptions() {
    return setupOptions;
  },
  get homeRegion() {
    return homeRegion;
  },
  get targetSeniority() {
    return targetSeniority;
  },
  get setupOpen() {
    return setupOpen;
  },
  get progress() {
    return progress;
  },
  get lastCall() {
    return lastCall;
  },
  get status() {
    return status;
  },
  get major() {
    return major;
  },
  get interests() {
    return interests;
  },
  get availableInterests() {
    return availableInterests;
  },
  get skills() {
    return skills;
  },
  get skillCatalog() {
    return skillCatalog;
  },
  get skillsSetupAt() {
    return skillsSetupAt;
  },
  /** True until the user has been through the skills form once. */
  get needsSkillsSetup() {
    return skillsSetupAt === null;
  },
  get skillsFormOpen() {
    return skillsFormOpen;
  },
  get loading() {
    return loading;
  },
  get refreshing() {
    return refreshing;
  },
  get phase() {
    return phase;
  },
  get planned() {
    return planned;
  },
  get scanRows() {
    return scanRows;
  },
  get pending() {
    return pending;
  },
  get error() {
    return error;
  },

  // --- live per-item state ---
  // The lists are raw snapshots, so `item.saved` is only what Rust knew when
  // it built the payload. These are the current answer, and they are the same
  // answer whichever list the row happens to be rendered from.

  isSaved(url: string): boolean {
    return savedUrls.has(url);
  },

  isSeen(url: string): boolean {
    return seenUrls.has(url);
  },

  /** Where an application stands, for a saved role that has one. */
  application(url: string): Application | null {
    return applications.get(url) ?? null;
  },

  hasSkill(name: string): boolean {
    return skillSet.has(name);
  },

  /**
   * The `n of m` skill coverage for a posting, or `null` when it named too few
   * skills for the figure to mean anything.
   *
   * Computed here rather than read off `matched_skills` so a skill chip tap
   * moves every figure on screen at once — the re-rank that makes it official
   * is a second behind, and waiting for it made the chips feel broken.
   */
  match(item: ScrapedItem): { have: number; total: number } | null {
    const required = item.required_skills;
    if (!required || required.length < MIN_SKILLS_TO_SHOW) return null;
    let have = 0;
    for (const skill of required) if (skillSet.has(skill)) have++;
    return { have, total: required.length };
  },

  /**
   * The full posting behind a list row, memoized.
   *
   * List rows carry no description — that is what keeps `get_internships` from
   * being a 45 MB payload — so the pane fetches the one posting it is showing.
   * Kept because the split pane re-mounts this on every arrow key, and a role
   * you click back to should not go blank again.
   */
  async detail(url: string): Promise<ScrapedItem | null> {
    const hit = detailCache.get(url);
    if (hit !== undefined) return hit;
    const fetched = await api.getItemDetail(url);
    // Bounded rather than unbounded: descriptions average 3.5 KB and a long
    // session clicking through a job board would otherwise keep every one.
    if (detailCache.size >= DETAIL_CACHE_LIMIT) detailCache.clear();
    detailCache.set(url, fetched);
    return fetched;
  },

  /** The cached posting, if the pane has already shown this one. */
  peekDetail(url: string): ScrapedItem | null | undefined {
    return detailCache.get(url);
  },

  /** The posting's skills split into the ones covered and the ones not. */
  splitSkills(item: ScrapedItem): { have: string[]; missing: string[] } {
    const have: string[] = [];
    const missing: string[] = [];
    for (const skill of item.required_skills ?? []) {
      (skillSet.has(skill) ? have : missing).push(skill);
    }
    return { have, missing };
  },

  /** Called once from the root layout. Idempotent across navigations. */
  async init() {
    if (started) return;
    started = true;
    watchScrolling();

    try {
      // Subscribe before the first read. The Rust side kicks off a background
      // refresh at startup and emits when it lands; registering afterwards
      // would drop that event on a fast machine and leave the feed empty until
      // a manual refresh.
      await Promise.all([
        listen<number>("feed:updated", (e) => {
          if (phase === "scraping") scanRows = e.payload;
          scheduleReload();
        }),
        listen<boolean>("feed:refreshing", (e) => {
          refreshing = e.payload;
          if (e.payload) {
            // A new refresh starts with an empty list.
            progress = [];
            planned = 0;
            scanRows = 0;
          } else {
            phase = "idle";
            // The end of a run releases anything that was waiting for it, and
            // owes the lists one last load to show where it ended up.
            for (const wake of idleWaiters.splice(0)) wake();
            scheduleReload();
          }
        }),
        listen<ScanPhase>("feed:phase", (e) => (phase = e.payload)),
        listen<number>("feed:plan", (e) => (planned = e.payload)),
        listen<ScrapeProgress>("feed:progress", (e) => (progress = [...progress, e.payload])),
      ]);

      const [
        savedMajor,
        chosen,
        offered,
        mySkills,
        catalog,
        setupAt,
        gate,
        wanted,
        options,
        region,
        levels,
      ] = await Promise.all([
        api.getSetting("major"),
        api.getInterests(),
        api.listInterests(),
        api.getSkills(),
        api.listSkills(),
        api.getSetting("skills_setup_at"),
        api.getOnboardingState(),
        api.getScrapePrefs(),
        api.listSetupOptions(),
        api.getSetting("home_region"),
        api.getSetting("target_seniority"),
      ]);
      onboarding = gate;
      prefs = wanted;
      setupOptions = options;
      homeRegion = region && region.trim() ? region : null;
      targetSeniority = parseList(levels);
      major = savedMajor ?? DEFAULT_MAJOR;
      interests = chosen;
      availableInterests = offered;
      skills = mySkills;
      reset(skillSet, mySkills);
      skillCatalog = catalog;
      skillsSetupAt = setupAt;

      await loadAll();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  },

  async setMajor(next: string) {
    const previous = major;
    major = next;
    try {
      await api.setSetting("major", next);
      await loadAll();
    } catch (e) {
      major = previous;
      toast.error("change your field", e);
    }
  },

  /** Re-ranks rather than re-scrapes: interests only affect scoring. */
  async setInterests(next: string[]) {
    const previous = interests;
    interests = next;
    try {
      await api.setInterests(next);
      await loadAll();
    } catch (e) {
      interests = previous;
      toast.error("save your interests", e);
    }
  },

  // --- skills ---

  openSkillsForm() {
    skillsFormOpen = true;
  },

  closeSkillsForm() {
    skillsFormOpen = false;
  },

  /**
   * Flips one skill. Persists straight away, but re-ranks on a trailing
   * debounce: a reload is four invokes and a full re-rank of the whole cache,
   * which is fine for the odd interest chip and wrong for skill chips tapped
   * in a burst while reading one posting — where it would also reshuffle the
   * list out from under the detail pane between taps.
   *
   * The coverage figures do not wait for it. They read `skillSet`, which is
   * updated here, so every `n of m` on screen moves on the tap itself.
   */
  async toggleSkill(name: string) {
    const next = skills.includes(name)
      ? skills.filter((s) => s !== name)
      : [...skills, name];
    const previous = skills;

    skills = next;
    reset(skillSet, next);

    try {
      await api.setSkills(next);
      scheduleRerank();
    } catch (e) {
      skills = previous;
      reset(skillSet, previous);
      toast.error("update your skills", e);
    }
  },

  /** Bulk write from the wizard. Re-ranks immediately — there is no burst. */
  async setSkills(next: string[]) {
    const previous = skills;
    skills = next;
    reset(skillSet, next);
    try {
      await api.setSkills(next);
      await loadAll();
    } catch (e) {
      skills = previous;
      reset(skillSet, previous);
      toast.error("save your skills", e);
    }
  },

  /** Marks the form as having been seen, which retires the setup prompt. */
  async completeSkillsSetup() {
    const at = new Date().toISOString();
    skillsSetupAt = at;
    try {
      await api.setSetting("skills_setup_at", at);
    } catch (e) {
      // Kept locally: the prompt is gone for this session either way, and it
      // is not worth stranding the reader in the form over a flag.
      toast.error("remember that skills are set up", e);
    }
  },

  // --- the setup form ---

  /** Saves the form's answers. Does not scrape: see `scrapeNow`. */
  async saveSetup(
    wanted: ScrapePrefs,
    chosenInterests: string[],
    region: string | null,
    levels: string[],
  ) {
    await api.completeOnboarding(wanted, chosenInterests, region, levels);
    prefs = wanted;
    interests = chosenInterests;
    homeRegion = region;
    targetSeniority = levels;
    // So the progress list can name the sources that are about to run.
    status = await api.feedStatus();
  },

  /**
   * Scrapes what was just asked for, resolving when the run has ended — which is
   * not when the feed becomes usable: the lists fill as sources finish, and the
   * caller can let the reader in before this resolves. Calls Rust directly
   * rather than `refresh`, which gives up if anything is already running: an
   * upgrader's launch scrape may still be in flight when they press Start, and
   * the right thing then is to follow it, not to return as though it had
   * finished.
   */
  async scrapeNow(): Promise<RefreshOutcome> {
    error = null;
    progress = [];
    refreshing = true;
    const outcome = await api.refreshFeed(true);
    if (outcome.status === "skipped") await whenIdle();
    await reloadNow();
    return outcome;
  },

  /** Dismisses the form, whichever way it was reached. */
  finishSetup() {
    onboarding = "done";
    setupOpen = false;
  },

  /** An existing install waving the form away. */
  async skipSetup() {
    await api.skipOnboarding();
    this.finishSetup();
  },

  openSetup() {
    setupOpen = true;
  },

  closeSetup() {
    setupOpen = false;
  },

  /**
   * Changes what is scraped, from Settings. Rust deletes whatever is no longer
   * wanted straight away; a plain refresh then fetches only what newly became
   * wanted, since everything else is still inside its freshness window.
   */
  async setPrefs(next: ScrapePrefs) {
    try {
      await api.setScrapePrefs(next);
      prefs = next;
      await loadAll();
      void this.refresh(false);
    } catch (e) {
      toast.error("change what is collected", e);
    }
  },

  /**
   * Starts a refresh. `force` ignores how recently each source ran.
   *
   * Whether one is running is the backend's to say (`feed:refreshing`), so this
   * only claims the flag for the instant before that event arrives, and gives
   * it back if nothing was started — the reader has not been asked yet, or
   * another refresh got there first.
   */
  async refresh(force = false) {
    if (refreshing) return;
    error = null;
    refreshing = true;
    try {
      const outcome = await api.refreshFeed(force);
      refreshing = outcome.status === "skipped" ? (await api.feedStatus()).refreshing : false;
      await reloadNow();
    } catch (e) {
      // A failed refresh keeps whatever is already cached on screen.
      error = String(e);
      refreshing = false;
    }
  },

  /**
   * Stops the running refresh. What it had already written stays, and sources
   * it had not reached stay due, so Start picks up where it left off.
   */
  async stop() {
    if (!refreshing || phase === "stopping") return;
    phase = "stopping";
    try {
      await api.stopRefresh();
    } catch (e) {
      toast.error("stop the refresh", e);
    }
  },

  /** Shows the results held back while the reader was mid-list. */
  applyPending() {
    scrolledAway = false;
    scheduleReload(true);
  },

  /** A page change resets what "scrolled away" means. */
  noteNavigated() {
    scrolledAway = false;
  },

  // --- item actions ---
  // Each updates the UI first and reconciles after, so starring a card feels
  // instant rather than waiting on a database round trip.

  async toggleSaved(item: ScrapedItem, { quiet = false }: { quiet?: boolean } = {}) {
    const next = !savedUrls.has(item.url);
    const previous = saved;
    if (next) savedUrls.add(item.url);
    else savedUrls.delete(item.url);
    saved = next
      ? [{ ...item, saved: true }, ...saved.filter((s) => s.url !== item.url)]
      : saved.filter((s) => s.url !== item.url);

    try {
      await api.setSaved(item, next);
      status = await api.feedStatus();
      if (!quiet) {
        if (next) toast.success("Saved");
        else
          toast.info("Removed from saved", {
            label: "Undo",
            run: () => this.toggleSaved(item, { quiet: true }),
          });
      }
    } catch (e) {
      // Put the optimistic change back — both the set and the list — if the
      // write failed, or the Saved page would keep showing what was not kept.
      if (next) savedUrls.delete(item.url);
      else savedUrls.add(item.url);
      saved = previous;
      toast.error(next ? "save it" : "remove it from saved", e);
    }
  },

  /**
   * Hides an item everywhere it is listed, with an Undo on the toast. The
   * Settings "restore hidden items" button is still there for the rest.
   */
  async dismiss(item: ScrapedItem) {
    const before = { items, internships, lastCall };
    items = items.filter((i) => i.url !== item.url);
    internships = internships.filter((i) => i.url !== item.url);
    lastCall = lastCall.filter((i) => i.url !== item.url);
    try {
      await api.setDismissed(item.url, true);
      status = await api.feedStatus();
      toast.info("Hidden from your feed", {
        label: "Undo",
        run: () => this.undismiss(item),
      });
    } catch (e) {
      ({ items, internships, lastCall } = before);
      toast.error("hide it", e);
    }
  },

  /** Brings one dismissed item back. The lists reload to put it in rank order. */
  async undismiss(item: ScrapedItem) {
    try {
      await api.setDismissed(item.url, false);
      await reloadNow();
    } catch (e) {
      toast.error("bring it back", e);
    }
  },

  async restoreDismissed() {
    try {
      await api.clearDismissed();
      await loadAll();
      toast.success("Hidden items restored");
    } catch (e) {
      toast.error("restore hidden items", e);
    }
  },

  /**
   * Wipes the local store and reloads the now-empty lists. The field,
   * interests and skills are untouched, so the next refresh ranks against the
   * same profile rather than starting from the defaults.
   */
  async clearData() {
    try {
      await api.clearData();
      error = null;
      toast.success("Cached data cleared");
    } catch (e) {
      toast.error("clear the cache", e);
    }
    try {
      await loadAll();
    } catch (e) {
      error = String(e);
    }
  },

  /** Closes the refresh-failed banner until the next failure. */
  clearError() {
    error = null;
  },

  /**
   * Sets where an application stands and its note. Tracking a role saves it
   * first, since the tracker lives on Saved and an unsaved row would be
   * pruned with the cache.
   */
  async setApplication(item: ScrapedItem, status: ApplicationStatus | null, note?: string | null) {
    const previous = applications.get(item.url) ?? null;
    const nextNote = note === undefined ? (previous?.note ?? null) : note;
    const statusChanged = (previous?.status ?? null) !== status;
    applications.set(item.url, {
      url: item.url,
      status,
      note: nextNote,
      updated_at: statusChanged ? new Date().toISOString() : (previous?.updated_at ?? null),
    });
    try {
      if (!savedUrls.has(item.url)) await this.toggleSaved(item, { quiet: true });
      await api.setApplication(item.url, status, nextNote);
    } catch (e) {
      if (previous) applications.set(item.url, previous);
      else applications.delete(item.url);
      toast.error("update the application", e);
    }
  },

  /** Records that the user opened an item, which sinks it in later rankings. */
  async markSeen(item: ScrapedItem) {
    if (seenUrls.has(item.url)) return;
    seenUrls.add(item.url);
    try {
      await api.markSeen(item.url);
    } catch {
      // Read tracking is a nicety; a failure here should not surface an error.
    }
  },
};
