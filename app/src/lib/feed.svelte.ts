import { listen } from "@tauri-apps/api/event";
import {
  feedStatus,
  getFeed,
  getInternships,
  getSetting,
  refreshFeed,
  setSetting,
  type FeedStatus,
  type ScrapedItem,
} from "./api";

const DEFAULT_MAJOR = "Software Engineering";

// One shared store rather than a fetch in every page: the feed and the
// internships hub read the same local database, and the header's refresh
// button has to drive both.

let items = $state<ScrapedItem[]>([]);
let internships = $state<ScrapedItem[]>([]);
let status = $state<FeedStatus | null>(null);
let major = $state(DEFAULT_MAJOR);
let loading = $state(true);
let refreshing = $state(false);
let error = $state<string | null>(null);
let started = false;

async function loadAll() {
  const [feedItems, internshipItems, st] = await Promise.all([
    getFeed(major),
    getInternships(major),
    feedStatus(),
  ]);
  items = feedItems;
  internships = internshipItems;
  status = st;
  refreshing = st.refreshing;
}

export const feed = {
  get items() {
    return items;
  },
  get internships() {
    return internships;
  },
  get status() {
    return status;
  },
  get major() {
    return major;
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

      major = (await getSetting("major")) ?? DEFAULT_MAJOR;
      await loadAll();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  },

  async setMajor(next: string) {
    major = next;
    await setSetting("major", next);
    await loadAll();
  },

  async refresh(force = false) {
    if (refreshing) return;
    error = null;
    refreshing = true;
    try {
      await refreshFeed(force);
      await loadAll();
    } catch (e) {
      // A failed refresh keeps whatever is already cached on screen.
      error = String(e);
    } finally {
      refreshing = false;
    }
  },
};
