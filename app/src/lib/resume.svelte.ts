import * as api from "./api";
import type { ResumeDoc, ResumeTheme, ResumeSummary, ResumeView, StoredResume } from "./api";

/**
 * The résumé the builder is editing, and the pages Rust laid out for it.
 *
 * Separate from `feed`: nothing here belongs to the feed's lifecycle, and that
 * store is already the busiest object in the app. The discipline is the same
 * though — **`view` is `$state.raw`**. A laid-out résumé is a few thousand
 * positioned boxes, and letting Svelte deep-proxy every one of them on every
 * keystroke is the frozen window the feed's own `$state.raw` comment warns
 * about. Pages are replaced whole, never edited in place.
 *
 * `doc` *is* a deep `$state`, deliberately: it is a few dozen objects the user
 * is directly editing, and two-way binding on a text field is worth more here
 * than the proxying costs.
 */

/** How long to sit still after a keystroke before asking Rust to lay out again. */
const RELAYOUT_DELAY = 250;
/** And before writing the row. Typing should not rewrite the database per key. */
const SAVE_DELAY = 1000;

let id = $state<string | null>(null);
let name = $state("My résumé");
let doc = $state<ResumeDoc | null>(null);
let theme = $state<ResumeTheme | null>(null);
let list = $state.raw<ResumeSummary[]>([]);
let themes = $state.raw<api.ResumeThemeOption[]>([]);

/** The laid-out pages. Raw: replaced whole, never mutated. */
let view = $state.raw<ResumeView | null>(null);

let loading = $state(true);
let saving = $state(false);
let error = $state<string | null>(null);
let dirty = $state(false);

let relayoutTimer: ReturnType<typeof setTimeout> | undefined;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
/** Guards against a slow layout landing on top of a newer one. */
let layoutToken = 0;
/** Whether the builder has already read a résumé this session; see `ensure`. */
let started = false;
/** The posting the preview is tailored against, if any. */
let against: string | undefined;

function fail(e: unknown, what: string) {
  console.error(what, e);
  error = e instanceof Error ? e.message : String(e);
}

async function relayout() {
  if (!id || !theme) return;
  const token = ++layoutToken;
  try {
    const next = await api.layoutResume({ id, url: against, theme });
    // A stale response must never replace a newer one: typing fires these
    // faster than they return, and out-of-order arrival would leave the page
    // showing the document as it was two keystrokes ago.
    if (token === layoutToken) view = next;
  } catch (e) {
    fail(e, "could not lay out the résumé");
  }
}

export const resume = {
  get id() {
    return id;
  },
  get name() {
    return name;
  },
  get doc() {
    return doc;
  },
  get theme() {
    return theme;
  },
  get list() {
    return list;
  },
  get themes() {
    return themes;
  },
  get view() {
    return view;
  },
  get loading() {
    return loading;
  },
  get saving() {
    return saving;
  },
  get dirty() {
    return dirty;
  },
  get error() {
    return error;
  },
  /** Whether there is a résumé at all yet — what the empty state keys off. */
  get exists() {
    return doc !== null;
  },

  /**
   * Reads the résumé once, and never again for the same session.
   *
   * The builder is a page the user leaves and comes back to — to look at a
   * posting, to check the tailored copy, to go and find a date. Re-reading the
   * row on every mount made that round trip lossy: the write is debounced by a
   * second, so leaving mid-edit and coming straight back put the stored copy
   * on screen, threw away whatever had not been written yet, and left the
   * pending save to store the version it had just clobbered. The document is
   * already in memory and already the newest one there is — coming back to it
   * should show it, not fetch over it.
   *
   * Same shape as `feed.init`, for the same reason: mount work that must
   * happen once, not once per navigation. `load` stays public for the cases
   * that genuinely want a re-read — an import, a delete.
   */
  async ensure() {
    if (started) return;
    started = true;
    await this.load();
    // A read that failed is not a load. Latching it would leave the builder
    // showing an error until the app restarted, with no way to try again.
    if (doc === null && error !== null) started = false;
  },

  /**
   * Loads the default résumé, or the one named, and lays it out.
   *
   * `url` points the preview at a posting: the same document, tailored. The
   * builder calls this without one.
   */
  async load(which?: string, url?: string) {
    loading = true;
    error = null;
    against = url;
    try {
      const [stored, summaries, themeOptions] = await Promise.all([
        api.getResume(which),
        api.listResumes(),
        themes.length > 0 ? Promise.resolve(themes) : api.listResumeThemes(),
      ]);
      list = summaries;
      themes = themeOptions;
      this.adopt(stored);
    } catch (e) {
      fail(e, "could not load the résumé");
    } finally {
      loading = false;
    }
  },

  /** Takes a freshly-read résumé as the one being edited. */
  adopt(stored: StoredResume | null) {
    if (!stored) {
      id = null;
      doc = null;
      theme = null;
      view = null;
      return;
    }
    id = stored.id;
    name = stored.name;
    doc = stored.doc;
    theme = stored.theme;
    dirty = false;
    void relayout();
  },

  /**
   * Records an edit: lays out again after a pause, saves after a longer one.
   *
   * Two timers rather than one because they answer different questions. The
   * preview has to track typing closely enough to feel live; the database does
   * not need a row rewritten per character.
   */
  touch() {
    dirty = true;
    clearTimeout(relayoutTimer);
    relayoutTimer = setTimeout(() => void relayout(), RELAYOUT_DELAY);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void resume.save(), SAVE_DELAY);
  },

  /** Writes the résumé, then re-reads it because Rust re-derives on save. */
  async save() {
    if (!doc || !theme) return;
    clearTimeout(saveTimer);
    saving = true;
    error = null;
    try {
      const created = id === null;
      const savedId = await api.saveResume(id, name, $state.snapshot(doc), $state.snapshot(theme));
      id = savedId;
      dirty = false;
      // Rust mints ids and re-reads every bullet's skills on save, so what
      // comes back is the authority. Keeping the local copy would leave a new
      // bullet with no id, and an id is what a per-posting override refers to.
      const stored = await api.getResume(savedId);
      if (stored) {
        doc = stored.doc;
        theme = stored.theme;
      }
      if (created) list = await api.listResumes();
      await relayout();
    } catch (e) {
      fail(e, "could not save the résumé");
    } finally {
      saving = false;
    }
  },

  rename(next: string) {
    name = next;
    this.touch();
  },

  setTheme(next: ResumeTheme) {
    theme = next;
    this.touch();
  },

  /** Starts a new, empty résumé and saves it, so it has an id immediately. */
  async create(newName = "My résumé") {
    started = true;
    id = null;
    name = newName;
    doc = emptyDoc();
    theme = themes[0]?.theme ?? null;
    await this.save();
  },

  /**
   * Empties the résumé being edited, keeping its id, name and design.
   *
   * The id is kept on purpose. It is what the row, the default flag and every
   * saved per-posting variant are keyed by, so starting over on the same id is
   * a document the user empties rather than a second row the builder can no
   * longer reach. Callers ask first — this does not.
   */
  async reset() {
    if (!doc) return;
    clearTimeout(relayoutTimer);
    clearTimeout(saveTimer);
    doc = emptyDoc();
    await this.save();
  },

  async remove(which: string) {
    await api.deleteResume(which);
    list = await api.listResumes();
    if (which === id) await this.load();
  },

  async makeDefault(which: string) {
    await api.setDefaultResume(which);
    list = await api.listResumes();
  },

  async importJson(json: string, importName?: string) {
    const newId = await api.importJsonResume(json, importName);
    // An import creates a *row*, and `get_resume` with no id answers with the
    // default one — so without this the builder would open the freshly
    // imported résumé now and the old one on the next visit, which looks
    // exactly like the import having been thrown away. There is no résumé
    // picker in the UI, so any row that is not the default is unreachable.
    await api.setDefaultResume(newId);
    started = true;
    await this.load(newId);
    return newId;
  },

  exportJson() {
    return api.exportJsonResume(id ?? undefined);
  },
};

/**
 * A blank résumé with the sections a software CV has, in the order the format
 * wants them. Mirrors `Resume::starter()` in Rust. Ids are left empty on
 * purpose: Rust mints them on save, which is the one place they come from.
 */
export function emptyDoc(): ResumeDoc {
  const section = (kind: api.SectionKind, title: string) => ({
    id: "",
    kind,
    title,
    entries: [],
  });
  return {
    contact: { name: "", headline: null, email: "", phone: "", location: "", links: [] },
    sections: [
      section("education", "Education"),
      section("experience", "Experience"),
      section("projects", "Projects"),
      section("skills", "Technical Skills"),
    ],
  };
}

/** A blank entry. Same rule about ids. */
export function emptyEntry(): api.ResumeEntry {
  return {
    id: "",
    org: "",
    title: "",
    location: "",
    start: "",
    end: "",
    link: null,
    detail: null,
    bullets: [],
  };
}

export function emptyBullet(): api.ResumeBullet {
  return { id: "", text: "", skills: [] };
}
