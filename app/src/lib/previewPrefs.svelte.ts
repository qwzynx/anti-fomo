/**
 * How the reader likes the résumé preview: one page at a time or a scroll
 * through all of them. Shared by the builder and the tailor screen, and kept
 * in `localStorage` because it is a per-device habit, not data — a store that
 * comes back empty simply means the default.
 */
export type PreviewMode = "paged" | "scroll";

const KEY = "anti-fomo.preview-mode";

function read(): PreviewMode {
  try {
    return localStorage.getItem(KEY) === "scroll" ? "scroll" : "paged";
  } catch {
    return "paged";
  }
}

let mode = $state<PreviewMode>(read());

export const previewPrefs = {
  get mode() {
    return mode;
  },
  set mode(next: PreviewMode) {
    mode = next;
    try {
      localStorage.setItem(KEY, next);
    } catch {
      // Private window or blocked storage: the choice lasts this session.
    }
  },
};
