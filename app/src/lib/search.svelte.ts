/**
 * The search palette: app-wide search, opened from the top bar or ⌘K/Ctrl-K
 * over whatever page you are on, so finding something never costs a
 * navigation.
 *
 * Roles also has its own field, because a job board without one is the first
 * thing anyone misses. The two share one matcher (`lib/query.ts`), and the
 * palette's "See all roles" hands its query to Roles rather than the two
 * keeping separate answers to the same question.
 */

let open = $state(false);
let query = $state("");
/** What had focus before the palette opened. */
let returnFocus: HTMLElement | null = null;

export const search = {
  get open() {
    return open;
  },
  get query() {
    return query;
  },
  set query(next: string) {
    query = next;
  },

  /**
   * Opens the palette. The last query is kept (and selected, so typing
   * replaces it) — reopening to refine a search should not start from nothing.
   */
  show(seed?: string) {
    if (seed !== undefined) query = seed;
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    open = true;
  },

  hide() {
    open = false;
    // Back to whatever opened it, as every dialog should.
    const target = returnFocus;
    returnFocus = null;
    queueMicrotask(() => target?.focus());
  },

  toggle() {
    open ? this.hide() : this.show();
  },
};
