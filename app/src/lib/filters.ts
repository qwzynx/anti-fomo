// The app has one filter surface, on Roles. These are its facets.
//
// Freshness, sort and discipline used to be duplicated across a select row on
// the feed, a sheet on phones and a panel on the hub; the redesign collapsed
// all three into one chip bar, which is why what is left here is a single set
// rather than a feed set and a hub set.

import type { ScrapedItem } from "./api";
import { companyOf, payAnnualEquivalent } from "./item";

export const FRESHNESS = [
  { label: "Any time", hours: Infinity },
  { label: "Last 24 hours", hours: 24 },
  { label: "Past week", hours: 24 * 7 },
  { label: "Past month", hours: 24 * 30 },
] as const;

export type FreshnessLabel = (typeof FRESHNESS)[number]["label"];

/** The fields the app knows how to rank for. Offered on Settings. */
export const DISCIPLINES = ["All", "Software Engineering"];

export function freshnessCutoff(label: string): number {
  const hours = FRESHNESS.find((f) => f.label === label)?.hours ?? Infinity;
  return hours === Infinity ? -Infinity : Date.now() - hours * 3600 * 1000;
}

// --- internships hub facets ---

export const SPECIALTIES = [
  "Frontend",
  "Backend",
  "Full-Stack",
  "DevOps",
  "AI/ML",
  "Embedded",
  "Data",
  "Product",
  "Security",
];

export const MODALITIES = ["All", "Remote", "Hybrid", "On-site"] as const;
export type Modality = (typeof MODALITIES)[number];

/** The two `item_type`s the hub ever shows — it already filters to `is_opportunity()`. */
export const TYPES = ["All", "Job", "Internship"] as const;
export type TypeFilter = (typeof TYPES)[number];

export const LOCATIONS = [
  "Canada",
  "USA",
  "Global / Multi-region",
  "Toronto",
  "Vancouver",
  "Waterloo",
  "San Francisco",
  "New York",
  "Seattle",
  "London",
];

/** Disciplines offered on the internships hub, which also surfaces "General". */
export const HUB_DISCIPLINES = ["All", "Software Engineering", "General"];

export const HUB_SORTS = [
  "Best match",
  "Last call",
  "Newest posted",
  "Closing soonest",
  "Closing latest",
  "Highest pay",
  "Company A–Z",
] as const;
export type HubSort = (typeof HUB_SORTS)[number];

/** One-line explanation shown under each option in the sort menu. */
export const HUB_SORT_BLURBS: Record<HubSort, string> = {
  "Best match": "Personalized to your interests and skills",
  "Last call": "Strong matches closing soon first, then everything else with a deadline",
  "Newest posted": "Most recently seen first",
  "Closing soonest": "Application deadlines coming up first",
  "Closing latest": "Application deadlines coming up last",
  "Highest pay": "Highest disclosed compensation first",
  "Company A–Z": "Alphabetical by employer",
};

/**
 * Orders a filtered result set for the hub. A posting missing the field a
 * sort keys on — no deadline, no disclosed pay, no recoverable company name —
 * sorts last regardless of direction: `closes_at`'s own contract is that
 * unknown is not the same as "furthest away", and the same logic applies to
 * pay and company here.
 */
export function sortItems(items: readonly ScrapedItem[], sort: HubSort): ScrapedItem[] {
  /**
   * Sorts on a key computed once per item rather than once per comparison.
   *
   * The comparator ran `new Date(…).getTime()`, `payAnnualEquivalent` or a
   * title split *inside* the compare — so on 17,739 roles each of those ran
   * roughly 2 n log n ≈ 500,000 times to order 17,739 things. A missing key
   * still sorts last regardless of direction: `closes_at`'s own contract is
   * that unknown is not the same as "furthest away", and the same logic
   * applies to pay and company here.
   */
  function byKey<T>(key: (item: ScrapedItem) => T | null, cmp: (a: T, b: T) => number) {
    const decorated = items.map((item) => ({ item, key: key(item) }));
    decorated.sort((a, b) => {
      if (a.key === null) return b.key === null ? 0 : 1;
      if (b.key === null) return -1;
      return cmp(a.key, b.key);
    });
    return decorated.map((d) => d.item);
  }

  const closesAt = (item: ScrapedItem) =>
    item.closes_at ? new Date(item.closes_at).getTime() : null;
  // A deadline already in the past is not "closing soon" — it is closed, and
  // ranking it first would put a dead posting above every one still open.
  // "Closing latest" has no such trap: an expired deadline is honestly the
  // least-far-out one, so it already falls out near the bottom on its own.
  const now = Date.now();
  const closesAtUpcoming = (item: ScrapedItem) => {
    const t = closesAt(item);
    return t !== null && t >= now ? t : null;
  };

  switch (sort) {
    case "Newest posted":
      return byKey((i) => i.timestamp, (a, b) => (a < b ? 1 : a > b ? -1 : 0));
    case "Last call": {
      // Three tiers, from what a row carries (`merit` is not in the list payload): the postings
      // Rust flagged as strong and closing, by date; then anything else with a live deadline,
      // by date; then everything with none (or already closed), by relevance. Within a tier
      // the better match wins a tie.
      const better = (i: ScrapedItem) => -(i.relevance_score ?? 0);
      return byKey(
        (i): [number, number, number] => {
          const t = closesAtUpcoming(i);
          if (t === null) return [2, 0, better(i)];
          return [i.last_call ? 0 : 1, t, better(i)];
        },
        (a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2],
      );
    }
    case "Closing soonest":
      return byKey(closesAtUpcoming, (a, b) => a - b);
    case "Closing latest":
      return byKey(closesAt, (a, b) => b - a);
    case "Highest pay":
      return byKey(payAnnualEquivalent, (a, b) => b - a);
    case "Company A–Z":
      return byKey(companyOf, (a, b) => a.localeCompare(b));
    default:
      return byKey((i) => i.relevance_score ?? 0, (a, b) => b - a);
  }
}

/**
 * What each specialty chip looks for, as whole words, compiled once.
 *
 * These were plain substrings of the chip's own label, which got both halves
 * wrong: "Data" fired on "database" and "metadata", "Product" on "production",
 * "AI/ML" only on the literal "ai/ml", and "Backend" missed "back-end". Each is
 * now the words a posting actually uses for the field, bounded so they cannot
 * fire inside another word.
 */
const SPECIALTY_PATTERNS: Record<string, RegExp> = {
  Frontend: /\b(front[- ]?end|ui engineer|web developer|react|angular|vue)\b/,
  Backend: /\b(back[- ]?end|server[- ]side|api engineer|platform engineer)\b/,
  "Full-Stack": /\b(full[- ]?stack)\b/,
  DevOps: /\b(dev[- ]?ops|site reliability|sre|infrastructure|cloud engineer|platform engineer)\b/,
  "AI/ML": /\b(ai|ml|a\.i\.|machine learning|deep learning|artificial intelligence|llms?|computer vision|nlp|data scien\w*)\b/,
  Embedded: /\b(embedded|firmware|fpga|rtos|hardware engineer|microcontroller\w*)\b/,
  Data: /\b(data (engineer\w*|scien\w*|analy\w*|platform|infrastructure)|analytics|business intelligence|etl)\b/,
  Product: /\b(product (manag\w*|design\w*|owner|analyst|engineer\w*)|pm intern|apm)\b/,
  Security: /\b(security|cyber\w*|infosec|appsec|penetration|soc analyst)\b/,
};

/** Whether a lowercased haystack is about `specialty`. */
export function matchesSpecialty(haystack: string, specialty: string): boolean {
  const pattern = SPECIALTY_PATTERNS[specialty];
  if (pattern) return pattern.test(haystack);
  const needle = specialty.toLowerCase();
  return haystack.includes(needle) || haystack.includes(needle.replace(/-/g, " "));
}

/** Toggles a value in a multi-select facet list. */
export function toggle<T>(list: T[], value: T): T[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}

// --- the Roles filter pass ---

/** Interest tags (Settings) that name a specialty chip, so quick chips follow the profile. */
export const INTEREST_SPECIALTY: Record<string, string> = {
  "AI/ML": "AI/ML",
  Frontend: "Frontend",
  Backend: "Backend",
  Security: "Security",
  Data: "Data",
  "DevOps/Cloud": "DevOps",
  Hardware: "Embedded",
  "Product/Design": "Product",
};

/** Floors offered for "pay at least", annual-equivalent. */
export const PAY_FLOORS = [40_000, 60_000, 80_000, 100_000, 150_000] as const;

export const DEADLINES = ["Any", "Has deadline", "Closing this week"] as const;

/** The places a role can be filtered to: the setup form's regions plus the multi-region tag. */
export function locationOptions(regions: string[]): string[] {
  return [...regions, "Global / Multi-region"];
}

/** A role's level: what Rust read off the title, or "Intern" for an internship that did not say. */
export function levelOf(item: ScrapedItem): string | null {
  return item.seniority ?? (item.item_type === "Internship" ? "Intern" : null);
}

export interface RoleFilterInput {
  type: TypeFilter;
  discipline: string;
  sources: string[];
  specialties: string[];
  modality: Modality;
  locations: string[];
  freshness: FreshnessLabel;
  seniority: string[];
  minPay: number | null;
  payDisclosed: boolean;
  deadline: (typeof DEADLINES)[number];
  goodMatchOnly: boolean;
}

/** The coverage a "good match" chip demands. */
export const GOOD_MATCH = 0.6;

/**
 * Applies every chip to a list, cheapest test first and computing nothing the
 * current chips do not ask for. Runs over the whole cache — 17,000 to 40,000
 * roles — on every tap, so anything per item here is paid that many times: no
 * strings unless a specialty chip is on, `Set`s for membership, and the clock
 * read once.
 */
export function filterRoles(
  items: readonly ScrapedItem[],
  f: RoleFilterInput,
  match: (item: ScrapedItem) => { have: number; total: number } | null,
  now = Date.now(),
): ScrapedItem[] {
  const cutoff = freshnessCutoff(f.freshness);
  const cutoffIso = cutoff === -Infinity ? null : new Date(cutoff).toISOString();
  const sourceSet = f.sources.length > 0 ? new Set(f.sources) : null;
  const locationSet = f.locations.length > 0 ? new Set(f.locations) : null;
  const levelSet = f.seniority.length > 0 ? new Set(f.seniority) : null;
  const wantedSpecialties = f.specialties.length > 0 ? f.specialties : null;
  const weekOut = now + 7 * 24 * 3600 * 1000;

  return items.filter((item) => {
    if (f.type !== "All" && item.item_type !== f.type) return false;
    if (f.discipline !== "All" && item.discipline !== f.discipline) return false;
    if (sourceSet && !sourceSet.has(item.source_platform)) return false;

    if (f.modality !== "All" || locationSet) {
      const tags = item.location_tags;
      if (!tags) return false;
      if (f.modality !== "All" && !tags.includes(f.modality)) return false;
      if (locationSet && !tags.some((t) => locationSet.has(t))) return false;
    }

    // ISO-8601 UTC strings order the same as the instants they name, so this
    // compares without parsing a Date per row.
    if (cutoffIso !== null && item.timestamp < cutoffIso) return false;

    if (levelSet) {
      const level = levelOf(item);
      if (!level || !levelSet.has(level)) return false;
    }

    if (f.minPay !== null || f.payDisclosed) {
      const pay = payAnnualEquivalent(item);
      if (pay === null) return false;
      if (f.minPay !== null && pay < f.minPay) return false;
    }

    if (f.deadline !== "Any") {
      if (!item.closes_at) return false;
      const t = new Date(item.closes_at).getTime();
      if (t < now) return false;
      if (f.deadline === "Closing this week" && t > weekOut) return false;
    }

    if (f.goodMatchOnly) {
      const m = match(item);
      if (!m || m.have / m.total < GOOD_MATCH) return false;
    }

    // Last, because it is the only test that has to build a string.
    if (wantedSpecialties) {
      const haystack = `${item.title} ${item.content_text ?? ""}`.toLowerCase();
      if (!wantedSpecialties.some((s) => matchesSpecialty(haystack, s))) return false;
    }
    return true;
  });
}

/** How many chips are narrowing the list (the query and sort are not chips). */
export function activeFilterCount(f: RoleFilterInput): number {
  return (
    (f.type !== "All" ? 1 : 0) +
    (f.discipline !== "All" ? 1 : 0) +
    f.sources.length +
    f.specialties.length +
    (f.modality !== "All" ? 1 : 0) +
    f.locations.length +
    (f.freshness !== "Any time" ? 1 : 0) +
    f.seniority.length +
    (f.minPay !== null ? 1 : 0) +
    (f.payDisclosed && f.minPay === null ? 1 : 0) +
    (f.deadline !== "Any" ? 1 : 0) +
    (f.goodMatchOnly ? 1 : 0)
  );
}

/**
 * Puts title and employer hits ahead of body hits when the sort is "Best
 * match" and a query is on; otherwise the sort's own order stands. Stable, so
 * relevance order survives within each tier.
 */
export function byQueryTier(sorted: ScrapedItem[], tier: Map<string, number>): ScrapedItem[] {
  if (tier.size === 0) return sorted;
  const head: ScrapedItem[] = [];
  const rest: ScrapedItem[] = [];
  for (const item of sorted) (tier.get(item.url) === 2 ? head : rest).push(item);
  return head.length === 0 || rest.length === 0 ? sorted : [...head, ...rest];
}
