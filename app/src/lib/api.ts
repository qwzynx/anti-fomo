import { invoke } from "@tauri-apps/api/core";

/**
 * One item, as the Rust `Item` struct defines it.
 *
 * Two shapes travel over `invoke`, and they are the same interface because
 * every field the lighter one omits is already optional here:
 *
 * - **List rows** (`get_feed`, `get_internships`) are Rust's `ListItem`: what a
 *   row renders and nothing else. No `description`/`requirements`/
 *   `responsibilities`/`perks`, no `tagged_skills`, no `score_breakdown`, and
 *   no `matched_skills` — the UI holds the live intersection, see
 *   `feed.match()`. Carrying the full item made `get_internships` a 45.6 MB
 *   payload for 17,739 rows; this one is 8.8 MB.
 * - **Whole items** (`get_saved`, `get_item_detail`) carry everything. The
 *   detail pane is the only place the fetched posting is on screen, so it is
 *   the only place that asks for it.
 *
 * A row's `saved`/`seen` are a snapshot of what Rust knew when it built the
 * payload. Nothing reads them directly — `feed.isSaved`/`feed.isSeen` are the
 * live answer, because the lists are `$state.raw` and a mutated field on a raw
 * item is invisible to the UI.
 */
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

/**
 * One posting in full — the fetched description and the score breakdown the
 * list payload leaves out. `null` once the cache has pruned it and it was
 * never saved.
 */
export const getItemDetail = (url: string) =>
  invoke<ScrapedItem | null>("get_item_detail", { url });

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

// --- résumé ---
// The Rust `resume` module's types, field-for-field. `layout_resume` returns
// positioned boxes rather than HTML because the same boxes are what the PDF
// writer draws — see `ResumePreview.svelte` and `resume/layout.rs`.

export type ResumeFamily = "serif" | "sans";
export type ResumeFontStyle = "regular" | "bold" | "italic" | "bolditalic";
export type ResumeThemeId = "classic" | "accent" | "modern" | "banner";
export type ResumePageSize = "letter" | "a4";
export type ResumeHeadingStyle = "rule" | "plain" | "band";
export type SectionKind =
  | "education"
  | "experience"
  | "projects"
  | "leadership"
  | "skills"
  | "awards"
  | "custom";

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/** One positioned box. `y` on a text box is the **baseline**, not the top. */
export type Draw =
  | {
      kind: "text";
      x: number;
      y: number;
      size: number;
      family: ResumeFamily;
      style: ResumeFontStyle;
      color: Rgb;
      /** Extra points per character. Only the name and the headings use it. */
      tracking: number;
      text: string;
    }
  | { kind: "rect"; x: number; y: number; w: number; h: number; color: Rgb }
  | { kind: "link"; x: number; y: number; w: number; h: number; url: string };

/** One page, in points. US Letter is 612 × 792. */
export interface ResumePage {
  width: number;
  height: number;
  items: Draw[];
}

export interface AccentRoles {
  name: boolean;
  headings: boolean;
  rules: boolean;
  org: boolean;
  bullet_marks: boolean;
}

export interface ResumeTheme {
  id: ResumeThemeId;
  page: ResumePageSize;
  family: ResumeFamily;
  base_size: number;
  leading: number;
  margin: number;
  accent: Rgb;
  heading: ResumeHeadingStyle;
  accent_roles: AccentRoles;
  max_pages: number;
}

export interface ResumeLink {
  label: string;
  url: string;
}

export interface ResumeContact {
  name: string;
  headline: string | null;
  email: string;
  phone: string;
  location: string;
  links: ResumeLink[];
}

export interface ResumeBullet {
  id: string;
  text: string;
  /** Catalog skills Rust read out of `text`. Recomputed on every save. */
  skills: string[];
}

export interface ResumeEntry {
  id: string;
  org: string;
  title: string;
  location: string;
  start: string;
  end: string;
  link: string | null;
  detail: string | null;
  bullets: ResumeBullet[];
}

export interface ResumeSection {
  id: string;
  kind: SectionKind;
  title: string;
  entries: ResumeEntry[];
}

export interface ResumeDoc {
  contact: ResumeContact;
  sections: ResumeSection[];
}

export interface StoredResume {
  id: string;
  name: string;
  doc: ResumeDoc;
  theme: ResumeTheme;
  is_default: boolean;
}

export interface ResumeSummary {
  id: string;
  name: string;
  is_default: boolean;
  updated_at: string;
}

export interface DroppedBullet {
  id: string;
  reason: string;
}

/**
 * A laid-out résumé. When it was laid out against a posting, it also carries
 * what the tailoring decided — `why` is the per-bullet evidence and `dropped`
 * is what the page budget cut, so the panel can show the trim rather than let
 * it look like data loss.
 */
export interface ResumeView {
  pages: ResumePage[];
  fill: number;
  bullets: string[];
  entries: string[];
  why: Record<string, string[]>;
  dropped: DroppedBullet[];
  covered: string[];
  required: string[];
  theme: ResumeTheme;
  filename: string;
}

export interface ResumeVariant {
  theme: ResumeTheme | null;
  headline: string | null;
  /** Entry or bullet id → forced in (and never trimmed) or out. */
  include: Record<string, boolean>;
  order: Record<string, string[]>;
  text: Record<string, string>;
  skills_lead: string[];
}

export interface ResumeThemeOption {
  id: ResumeThemeId;
  label: string;
  theme: ResumeTheme;
}

export const listResumes = () => invoke<ResumeSummary[]>("list_resumes");

/** `id` omitted means the default résumé. */
export const getResume = (id?: string) =>
  invoke<StoredResume | null>("get_resume", { id: id ?? null });

export const saveResume = (
  id: string | null,
  name: string,
  doc: ResumeDoc,
  theme: ResumeTheme,
) => invoke<string>("save_resume", { id, name, doc, theme });

export const deleteResume = (id: string) => invoke<void>("delete_resume", { id });

export const setDefaultResume = (id: string) => invoke<void>("set_default_resume", { id });

export const getResumeVariant = (url: string, resumeId: string) =>
  invoke<ResumeVariant | null>("get_resume_variant", { url, resumeId });

export const saveResumeVariant = (url: string, resumeId: string, variant: ResumeVariant) =>
  invoke<void>("save_resume_variant", { url, resumeId, variant });

export const clearResumeVariant = (url: string, resumeId: string) =>
  invoke<void>("clear_resume_variant", { url, resumeId });

/**
 * The pages to draw. With a `url` the layout is tailored to that posting and
 * the result carries the tailoring's reasoning; without one it is the builder's
 * own preview with everything included.
 */
export const layoutResume = (opts: {
  id?: string;
  url?: string;
  theme?: ResumeTheme;
}) =>
  invoke<ResumeView | null>("layout_resume", {
    id: opts.id ?? null,
    url: opts.url ?? null,
    theme: opts.theme ?? null,
  });

/** The PDF itself, as raw bytes. */
export const renderResumePdf = (opts: { id?: string; url?: string; theme?: ResumeTheme }) =>
  invoke<ArrayBuffer>("render_resume_pdf", {
    id: opts.id ?? null,
    url: opts.url ?? null,
    theme: opts.theme ?? null,
  });

export const importJsonResume = (json: string, name?: string) =>
  invoke<string>("import_json_resume", { json, name: name ?? null });

export const exportJsonResume = (id?: string) =>
  invoke<string>("export_json_resume", { id: id ?? null });

export const listResumeThemes = () => invoke<ResumeThemeOption[]>("list_resume_themes");

/** `rgb(r g b)` for a colour that came from Rust. */
export function cssColor(c: Rgb): string {
  return `rgb(${c.r} ${c.g} ${c.b})`;
}
