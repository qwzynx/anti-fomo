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
  function byKey<T>(key: (item: ScrapedItem) => T | null, cmp: (a: T, b: T) => number) {
    return (a: ScrapedItem, b: ScrapedItem) => {
      const ka = key(a);
      const kb = key(b);
      if (ka === null && kb === null) return 0;
      if (ka === null) return 1;
      if (kb === null) return -1;
      return cmp(ka, kb);
    };
  }

  const closesAt = (item: ScrapedItem) => (item.closes_at ? new Date(item.closes_at).getTime() : null);
  // A deadline already in the past is not "closing soon" — it is closed, and
  // ranking it first would put a dead posting above every one still open.
  // "Closing latest" has no such trap: an expired deadline is honestly the
  // least-far-out one, so it already falls out near the bottom on its own.
  const closesAtUpcoming = (item: ScrapedItem) => {
    const t = closesAt(item);
    return t !== null && t >= Date.now() ? t : null;
  };

  switch (sort) {
    case "Newest posted":
      return [...items].sort((a, b) => b.timestamp.localeCompare(a.timestamp));
    case "Closing soonest":
      return [...items].sort(byKey(closesAtUpcoming, (a, b) => a - b));
    case "Closing latest":
      return [...items].sort(byKey(closesAt, (a, b) => b - a));
    case "Highest pay":
      return [...items].sort(byKey(payAnnualEquivalent, (a, b) => b - a));
    case "Company A–Z":
      return [...items].sort(byKey(companyOf, (a, b) => a.localeCompare(b)));
    default:
      return [...items].sort((a, b) => (b.relevance_score ?? 0) - (a.relevance_score ?? 0));
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
