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
export function sortItems(items: ScrapedItem[], sort: HubSort): ScrapedItem[] {
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

/** Postings write "full-stack" and "full stack" interchangeably. */
export function matchesSpecialty(haystack: string, specialty: string): boolean {
  const needle = specialty.toLowerCase();
  return haystack.includes(needle) || haystack.includes(needle.replace(/-/g, " "));
}

/** Toggles a value in a multi-select facet list. */
export function toggle<T>(list: T[], value: T): T[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}
