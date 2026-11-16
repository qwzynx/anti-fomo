/**
 * The small parts of a résumé entry that a tailored version can leave out one
 * at a time: the items of a detail line (a project's stack, "GPA 3.9 · Dean's
 * List") and the skills on a skills line.
 *
 * These mirror `resume::model::detail_items`, `detail_key` and
 * `tailor::skill_key` in Rust, which apply the exclusions — the two must split
 * a line the same way, or a chip switched off here would name an item Rust
 * never sees.
 */

/** A detail line's items, split on the first of `·`, `|`, `;`, `,` it uses. */
export function detailItems(detail: string | null | undefined): string[] {
  const text = detail ?? "";
  for (const sep of ["·", "|", ";", ","]) {
    if (text.includes(sep)) {
      return text
        .split(sep)
        .map((s) => s.trim())
        .filter(Boolean);
    }
  }
  const one = text.trim();
  return one ? [one] : [];
}

/** A skills line's run, which Rust always splits on commas. */
export function skillItems(run: string | null | undefined): string[] {
  return (run ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

export const detailKey = (entryId: string, item: string) => `${entryId}/detail:${item}`;

export const skillKey = (skill: string) => `skill:${skill}`;
