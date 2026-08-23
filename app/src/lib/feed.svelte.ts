import { listen } from "@tauri-apps/api/event";
// Namespaced: several store methods below intentionally share a name with the
// command they wrap, and `api.setSaved(…)` makes it obvious which is which.
import * as api from "./api";
import type { FeedStatus, ScrapedItem, SkillCategory } from "./api";

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

let items = $state<ScrapedItem[]>([]);
let internships = $state<ScrapedItem[]>([]);
let saved = $state<ScrapedItem[]>([]);
let status = $state<FeedStatus | null>(null);
let major = $state(DEFAULT_MAJOR);
let interests = $state<string[]>([]);
let availableInterests = $state<string[]>([]);
let skills = $state<string[]>([]);
let skillCatalog = $state<SkillCategory[]>([]);
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
}

/** Applies a change to the same item wherever it appears across the lists. */
function patch(url: string, change: (item: ScrapedItem) => void) {
  for (const list of [items, internships, saved]) {
    for (const item of list) {
      if (item.url === url) change(item);
    }
  }
}

/**
 * Recomputes every item's `matched_skills` against the current profile without
 * a round trip. `required_skills` is already on each item, so the intersection
 * Rust would compute is one the UI can do itself — which is what lets a chip
 * tap update the match figure instantly while the re-rank waits.
 */
function restampMatchedSkills() {
  const have = new Set(skills);
  for (const list of [items, internships, saved]) {
    for (const item of list) {
      if (!item.required_skills?.length) continue;
      item.matched_skills = item.required_skills.filter((s) => have.has(s));
    }
  }
}

/** Re-ranks once the taps stop, so the list does not reshuffle mid-burst. */
function scheduleRerank() {
  if (skillRerankTimer) clearTimeout(skillRerankTimer);
  skillRerankTimer = setTimeout(() => {
    skillRerankTimer = null;
    void loadAll();
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
        listen<number>("feed:updated", () => void loadAll()),
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
   * debounce: `loadAll()` is four invokes and a full re-rank of the whole
   * cache, which is fine for the odd interest chip and wrong for skill chips
   * tapped in a burst while reading one posting — where it would also
   * reshuffle the list out from under the detail pane between taps.
   */
  async toggleSkill(name: string) {
    const next = skills.includes(name)
      ? skills.filter((s) => s !== name)
      : [...skills, name];
    const previous = skills;

    skills = next;
    restampMatchedSkills();

    try {
      await api.setSkills(next);
      scheduleRerank();
    } catch (e) {
      skills = previous;
      restampMatchedSkills();
      error = String(e);
    }
  },

  /** Bulk write from the wizard. Re-ranks immediately — there is no burst. */
  async setSkills(next: string[]) {
    skills = next;
    restampMatchedSkills();
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
    const next = !item.saved;
    patch(item.url, (i) => (i.saved = next));
    saved = next
      ? [{ ...item, saved: true }, ...saved.filter((s) => s.url !== item.url)]
      : saved.filter((s) => s.url !== item.url);

    try {
      await api.setSaved(item, next);
      status = await api.feedStatus();
    } catch (e) {
      // Put the optimistic change back if the write failed.
      patch(item.url, (i) => (i.saved = !next));
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
    if (item.seen) return;
    patch(item.url, (i) => (i.seen = true));
    try {
      await api.markSeen(item.url);
    } catch {
      // Read tracking is a nicety; a failure here should not surface an error.
    }
  },
};
