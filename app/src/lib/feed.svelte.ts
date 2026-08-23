import { listen } from "@tauri-apps/api/event";
import { SvelteSet } from "svelte/reactivity";
// Namespaced: several store methods below intentionally share a name with the
// command they wrap, and `api.setSaved(…)` makes it obvious which is which.
import * as api from "./api";
import type { FeedStatus, ScrapedItem, SkillCategory } from "./api";
import { MIN_SKILLS_TO_SHOW } from "./skills";

const DEFAULT_MAJOR = "Software Engineering";

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
let status = $state<FeedStatus | null>(null);
let major = $state(DEFAULT_MAJOR);
let interests = $state.raw<string[]>([]);
let availableInterests = $state.raw<string[]>([]);
let skills = $state.raw<string[]>([]);
let skillCatalog = $state.raw<SkillCategory[]>([]);

/** Live per-item state, keyed by URL, shared across every list an item is in. */
const savedUrls = new SvelteSet<string>();
const seenUrls = new SvelteSet<string>();
/** The profile as a set, so a coverage check is a lookup and not a scan. */
const skillSet = new SvelteSet<string>();

// null until the user has been through the form once. A stored flag rather
// than "is the skill list empty", because finishing the form having picked
// nothing and never having opened it must look different — otherwise the
// setup prompt never goes away.
let skillsSetupAt = $state<string | null>(null);
let skillsFormOpen = $state(false);
let loading = $state(true);
let refreshing = $state(false);
let error = $state<string | null>(null);
let started = false;
let skillRerankTimer: ReturnType<typeof setTimeout> | null = null;
/** Coalesces the burst of `feed:updated` events one refresh emits. */
let reloadTimer: ReturnType<typeof setTimeout> | null = null;
let reloading = false;

/** Postings the detail pane has already fetched, keyed by URL. */
const detailCache = new Map<string, ScrapedItem | null>();
const DETAIL_CACHE_LIMIT = 200;

/** Replaces a set's contents in one pass, without notifying per element. */
function reset(set: SvelteSet<string>, values: Iterable<string>) {
  set.clear();
  for (const value of values) set.add(value);
}

async function loadAll() {
  const [feedItems, internshipItems, savedItems, st] = await Promise.all([
    api.getFeed(major),
    api.getInternships(major),
    api.getSaved(),
    api.feedStatus(),
  ]);
  items = feedItems;
  internships = internshipItems;
  saved = savedItems;
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
  for (const list of [feedItems, internshipItems, savedItems]) {
    for (const item of list) if (item.seen) seen.push(item.url);
  }
  reset(seenUrls, seen);
}

/**
 * Reloads on a trailing debounce.
 *
 * One refresh emits `feed:updated` twice — once when the scrape lands and once
 * when enrichment does — and a settings change can land on top of either. Each
 * reload is four commands and a full re-rank, so running them back to back
 * bought nothing but the second answer.
 */
function scheduleReload() {
  if (reloadTimer) clearTimeout(reloadTimer);
  reloadTimer = setTimeout(() => {
    reloadTimer = null;
    if (reloading) return;
    reloading = true;
    void loadAll()
      .catch((e) => (error = String(e)))
      .finally(() => (reloading = false));
  }, 150);
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

    try {
      // Subscribe before the first read. The Rust side kicks off a background
      // refresh at startup and emits when it lands; registering afterwards
      // would drop that event on a fast machine and leave the feed empty until
      // a manual refresh.
      await Promise.all([
        listen<number>("feed:updated", () => scheduleReload()),
        listen<boolean>("feed:refreshing", (e) => (refreshing = e.payload)),
      ]);

      const [savedMajor, chosen, offered, mySkills, catalog, setupAt] = await Promise.all([
        api.getSetting("major"),
        api.getInterests(),
        api.listInterests(),
        api.getSkills(),
        api.listSkills(),
        api.getSetting("skills_setup_at"),
      ]);
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
    major = next;
    await api.setSetting("major", next);
    await loadAll();
  },

  /** Re-ranks rather than re-scrapes: interests only affect scoring. */
  async setInterests(next: string[]) {
    interests = next;
    await api.setInterests(next);
    await loadAll();
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
      error = String(e);
    }
  },

  /** Bulk write from the wizard. Re-ranks immediately — there is no burst. */
  async setSkills(next: string[]) {
    skills = next;
    reset(skillSet, next);
    await api.setSkills(next);
    await loadAll();
  },

  /** Marks the form as having been seen, which retires the setup prompt. */
  async completeSkillsSetup() {
    const at = new Date().toISOString();
    skillsSetupAt = at;
    await api.setSetting("skills_setup_at", at);
  },

  async refresh(force = false) {
    if (refreshing) return;
    error = null;
    refreshing = true;
    try {
      await api.refreshFeed(force);
      await loadAll();
    } catch (e) {
      // A failed refresh keeps whatever is already cached on screen.
      error = String(e);
    } finally {
      refreshing = false;
    }
  },

  // --- item actions ---
  // Each updates the UI first and reconciles after, so starring a card feels
  // instant rather than waiting on a database round trip.

  async toggleSaved(item: ScrapedItem) {
    const next = !savedUrls.has(item.url);
    if (next) savedUrls.add(item.url);
    else savedUrls.delete(item.url);
    saved = next
      ? [{ ...item, saved: true }, ...saved.filter((s) => s.url !== item.url)]
      : saved.filter((s) => s.url !== item.url);

    try {
      await api.setSaved(item, next);
      status = await api.feedStatus();
    } catch (e) {
      // Put the optimistic change back if the write failed.
      if (next) savedUrls.delete(item.url);
      else savedUrls.add(item.url);
      error = String(e);
    }
  },

  /** Hides an item everywhere. Undoable only via Settings → restore. */
  async dismiss(item: ScrapedItem) {
    items = items.filter((i) => i.url !== item.url);
    internships = internships.filter((i) => i.url !== item.url);
    try {
      await api.setDismissed(item.url, true);
      status = await api.feedStatus();
    } catch (e) {
      error = String(e);
      await loadAll();
    }
  },

  async restoreDismissed() {
    await api.clearDismissed();
    await loadAll();
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
    } catch (e) {
      error = String(e);
    }
    await loadAll();
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
