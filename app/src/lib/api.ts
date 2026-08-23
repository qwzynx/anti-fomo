import { invoke } from "@tauri-apps/api/core";

export interface ScrapedItem {
  title: string;
  source_platform: string;
  item_type: "Job" | "Internship" | "Article" | "Event";
  url: string;
  content_text: string;
  timestamp: string;
  discipline: string | null;
  relevance_score: number | null;
  location?: string | null;
  location_tags?: string[];
  /** The simplify.jobs posting id, when the source supplied one. */
  simplify_id?: string | null;
  /** The employer, as its own field rather than a fragment of `title`. */
  company?: string | null;
  /**
   * When applications close, RFC 3339. Most postings publish no deadline, and
   * `null` means exactly that — unknown, not "closes today". Every sort over
   * this puts unknown last rather than coercing it to a date.
   */
  closes_at?: string | null;
  /** Published compensation only. Nothing here is ever estimated. */
  salary_min?: number | null;
  salary_max?: number | null;
  salary_currency?: string | null;
  /** "year" | "month" | "week" | "day" | "hour". */
  salary_period?: string | null;
  /** Intern | New grad | Junior | Mid | Senior | Lead, read off the title. */
  seniority?: string | null;
  /** Which of the user's interest tags fired, so a card can explain its rank. */
  matched_interests?: string[];
  /** Catalog skills this posting asks for. Opportunities only. */
  required_skills?: string[];
  /** The subset of `required_skills` the user has declared. */
  matched_skills?: string[];
  /** 1-4, strongest first, with the user's per-company overrides applied. */
  company_tier?: number | null;
  /** Every scoring term that fired, for the "why this ranks here" panel. */
  score_breakdown?: ScoreTerm[];
  /**
   * The posting as the employer wrote it, once the enrichment pass has fetched
   * it. Absent where it could not, which is what makes the detail pane explain
   * itself instead of showing a skills panel — see `hasScrapedPosting` in
   * `lib/item`. All four fields hold newline-separated lines.
   */
  description?: string | null;
  requirements?: string | null;
  responsibilities?: string | null;
  perks?: string | null;
  /** Skills the source itself tagged the posting with. */
  tagged_skills?: string[];
  saved?: boolean;
  seen?: boolean;
}

/** One term of the relevance score. `label` is display text, not an id. */
export interface ScoreTerm {
  label: string;
  points: number;
}

/** One category of the Rust-owned skill catalog. */
export interface SkillCategory {
  name: string;
  skills: string[];
}

export interface SourceHealth {
  name: string;
  count: number;
}

export interface FeedStatus {
  last_refresh: string | null;
  item_count: number;
  refreshing: boolean;
  stale: boolean;
  saved_count: number;
  dismissed_count: number;
  sources: SourceHealth[];
}

// --- Rust commands ---
// These replace the HTTP endpoints the Next.js app called. There is no server,
// no base URL and no auth token: the scraping and ranking happen in-process.

export const getFeed = (major?: string) =>
  invoke<ScrapedItem[]>("get_feed", { major: major ?? null });

export const getInternships = (major?: string) =>
  invoke<ScrapedItem[]>("get_internships", { major: major ?? null });

export const getSaved = () => invoke<ScrapedItem[]>("get_saved");

export const feedStatus = () => invoke<FeedStatus>("feed_status");

// --- user actions ---
// The whole item goes with a save so Rust can snapshot it, letting the saved
// list outlive the item cache being pruned.

export const setSaved = (item: ScrapedItem, saved: boolean) =>
  invoke<void>("set_saved", { url: item.url, saved, item: saved ? item : null });

export const setDismissed = (url: string, dismissed: boolean) =>
  invoke<void>("set_dismissed", { url, dismissed });

export const markSeen = (url: string) => invoke<void>("mark_seen", { url });

export const clearDismissed = () => invoke<void>("clear_dismissed");

/** Empties the local store. The profile in `settings` is deliberately kept. */
export const clearData = () => invoke<void>("clear_data");

// --- interests ---

export const listInterests = () => invoke<string[]>("list_interests");

export const getInterests = () => invoke<string[]>("get_interests");

export const setInterests = (interests: string[]) =>
  invoke<void>("set_interests", { interests });

// --- skills ---

export const listSkills = () => invoke<SkillCategory[]>("list_skills");

export const getSkills = () => invoke<string[]>("get_skills");

export const setSkills = (skills: string[]) => invoke<void>("set_skills", { skills });

/** Returns the item count written, or null when the cache was still fresh. */
export const refreshFeed = (force = false) => invoke<number | null>("refresh", { force });

export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });

export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });

export const listSources = () => invoke<string[]>("list_sources");

// --- display helpers ---

export function domainOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
}

/** Company/site logo via Google's public favicon service. */
export function logoFor(url: string): string | null {
  const domain = domainOf(url);
  if (!domain) return null;
  return `https://www.google.com/s2/favicons?domain=${domain}&sz=64`;
}

export function timeAgo(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "";
  const mins = Math.round((Date.now() - then) / 60000);
  if (Math.abs(mins) < 60) return mins <= 1 ? "just now" : `${mins}m ago`;
  const hours = Math.round(mins / 60);
  if (Math.abs(hours) < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(iso).toLocaleDateString();
}
