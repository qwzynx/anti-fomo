// The app has one filter surface, on Roles. These are its facets.
//
// Freshness, sort and discipline used to be duplicated across a select row on
// the feed, a sheet on phones and a panel on the hub; the redesign collapsed
// all three into one chip bar, which is why what is left here is a single set
// rather than a feed set and a hub set.

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

export const HUB_SORTS = ["Relevance", "Newest first", "Company name"] as const;
export type HubSort = (typeof HUB_SORTS)[number];

/** Postings write "full-stack" and "full stack" interchangeably. */
export function matchesSpecialty(haystack: string, specialty: string): boolean {
  const needle = specialty.toLowerCase();
  return haystack.includes(needle) || haystack.includes(needle.replace(/-/g, " "));
}

/** Toggles a value in a multi-select facet list. */
export function toggle<T>(list: T[], value: T): T[] {
  return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
}
