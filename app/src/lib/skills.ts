import type { SkillCategory } from "./api";

// Presentation only. The skill names and their categories are Rust-owned and
// arrive through `list_skills`, because a skill the extractor has no keywords
// for would sit in the picker and never match anything. What lives here is how
// those categories are paced across the wizard's steps.

/**
 * How the ten catalog categories are grouped into wizard steps. Ten steps of
 * "Next" is a worse first run than six, and the pairs are chosen so each step
 * is one decision the user can hold in their head at once.
 *
 * Categories are matched by name; any the Rust catalog adds later that no step
 * claims are appended as a final step of their own rather than being dropped.
 */
export const WIZARD_STEPS: { title: string; blurb: string; categories: string[] }[] = [
  {
    title: "Languages",
    blurb: "The ones you'd be comfortable being handed a ticket in.",
    categories: ["Languages"],
  },
  {
    title: "Interfaces",
    blurb: "What you can build that someone looks at.",
    categories: ["Frontend", "Mobile"],
  },
  {
    title: "Servers & APIs",
    blurb: "What you can build that something else talks to.",
    categories: ["Backend & APIs"],
  },
  {
    title: "Data & models",
    blurb: "Where you can store it, move it, and learn from it.",
    categories: ["Data & Databases", "AI / ML"],
  },
  {
    title: "Shipping & safety",
    blurb: "Getting it running, and keeping it locked.",
    categories: ["Cloud & DevOps", "Security"],
  },
  {
    title: "Depth & craft",
    blurb: "The layers underneath, and how you work.",
    categories: ["Systems & Low-level", "Tools & Practices"],
  },
];

/**
 * Resolves the step plan against the catalog Rust actually sent, so a category
 * renamed or added on the Rust side degrades to an extra step instead of
 * silently disappearing from the wizard.
 */
export function stepsFor(catalog: SkillCategory[]) {
  const claimed = new Set(WIZARD_STEPS.flatMap((s) => s.categories));
  const byName = new Map(catalog.map((c) => [c.name, c]));

  const steps = WIZARD_STEPS.map((step) => ({
    title: step.title,
    blurb: step.blurb,
    groups: step.categories.map((name) => byName.get(name)).filter((c) => c !== undefined),
  })).filter((step) => step.groups.length > 0);

  const orphans = catalog.filter((c) => !claimed.has(c.name));
  if (orphans.length > 0) {
    steps.push({ title: "Everything else", blurb: "A few more worth claiming.", groups: orphans });
  }
  return steps;
}

/** Groups a flat skill list back into catalog order, dropping empty categories. */
export function groupByCategory(
  skills: string[],
  catalog: SkillCategory[],
): { name: string; skills: string[] }[] {
  const wanted = new Set(skills);
  return catalog
    .map((category) => ({
      name: category.name,
      skills: category.skills.filter((s) => wanted.has(s)),
    }))
    .filter((group) => group.skills.length > 0);
}

/**
 * Whether a posting has named enough skills for a match figure to mean
 * anything. Mirrors `MIN_SKILLS_TO_SCORE` in `rank.rs`: below it, Rust awards
 * no coverage bonus, so showing a score would promise a precision the ranking
 * does not have.
 *
 * The figure itself is `feed.match()`. It used to be a `skillMatch(item)` here
 * reading `item.matched_skills`, which meant every list had to be walked and
 * re-stamped on every skill tap; the store keeps the profile as a set and
 * intersects on read instead.
 */
export const MIN_SKILLS_TO_SHOW = 2;
