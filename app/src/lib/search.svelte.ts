/**
 * The one search surface in the app.
 *
 * The feed, the jobs hub and the saved list each used to carry their own
 * search box, which meant three fields that looked identical, searched
 * different things, and were all invisible until you scrolled to them. There
 * is one now, it lives in the top bar as app chrome, and it opens over
 * whatever page you are on — so searching never costs a navigation.
 */

let open = $state(false);
let query = $state("");

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

  /** Opens the palette, optionally seeded — the phone header passes nothing. */
  show(seed = "") {
    query = seed;
    open = true;
  },

  hide() {
    open = false;
  },

  toggle() {
    open ? this.hide() : this.show();
  },
};
