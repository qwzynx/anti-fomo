import { listen } from "@tauri-apps/api/event";
// Namespaced: several store methods below intentionally share a name with the
// command they wrap, and `api.setSaved(…)` makes it obvious which is which.
import * as api from "./api";
import type { FeedStatus, ScrapedItem } from "./api";

const DEFAULT_MAJOR = "Software Engineering";

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
let loading = $state(true);
let refreshing = $state(false);
let error = $state<string | null>(null);
let started = false;

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

      const [savedMajor, chosen, offered] = await Promise.all([
        api.getSetting("major"),
        api.getInterests(),
        api.listInterests(),
      ]);
      major = savedMajor ?? DEFAULT_MAJOR;
      interests = chosen;
      availableInterests = offered;

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
