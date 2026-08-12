import type { ScrapedItem } from "./api";

// Shared by the feed and the internships hub, which each had their own copy in
// the React version.

export const FRESHNESS = [
  { label: "Any time", hours: Infinity },
  { label: "Last 24 hours", hours: 24 },
  { label: "Past week", hours: 24 * 7 },
  { label: "Past month", hours: 24 * 30 },
] as const;

export type FreshnessLabel = (typeof FRESHNESS)[number]["label"];

export const SORTS = ["Relevance", "Newest first"] as const;
export type Sort = (typeof SORTS)[number];

export const DISCIPLINES = ["All", "Software Engineering"];

/** Sources whose articles count as tech news for the Trending widget. */
export const NEWS_SOURCES = [
  "Hacker News",
  "Phoronix",
  "TLDR Tech",
  "HN Top Links",
  "Daily.dev",
];

export const TYPE_OPTIONS = ["All", "Internships", "Events", "Articles"] as const;
export type TypeOption = (typeof TYPE_OPTIONS)[number];

export const TYPE_MAP: Record<string, ScrapedItem["item_type"][]> = {
  Internships: ["Internship", "Job"],
  Events: ["Event"],
  Articles: ["Article"],
};

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
