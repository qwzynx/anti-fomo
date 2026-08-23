import { domainOf, type ScrapedItem } from "./api";

/** Per-type badge colours, resolved through the theme tokens in app.css. */
export function typeBadgeClass(type: ScrapedItem["item_type"]): string {
  switch (type) {
    case "Internship":
    case "Job":
      return "bg-job-soft text-job";
    case "Event":
      return "bg-event-soft text-event";
    default:
      return "bg-article-soft text-article";
  }
}

/**
 * Splits "Role at Company" style titles for a stronger visual hierarchy.
 *
 * The role leads and the company follows. Someone scanning a list of postings
 * is looking for the job, not the employer — twenty rows that all say the
 * company first make the reader work through a column of names to find the one
 * thing that distinguishes them.
 */
export function splitTitle(item: ScrapedItem): { primary: string; secondary: string | null } {
  if (item.item_type === "Internship" || item.item_type === "Job") {
    const idx = item.title.lastIndexOf(" at ");
    if (idx > 0) {
      return {
        primary: item.title.slice(0, idx),
        secondary: item.title.slice(idx + 4),
      };
    }
  }
  return { primary: item.title, secondary: null };
}

/**
 * The employer a role sorts and groups by. Sources that know it say so;
 * everything else falls back to the same "at Company" split the title
 * already renders, rather than a second, divergent guess.
 */
export function companyOf(item: ScrapedItem): string | null {
  return item.company?.trim() || splitTitle(item).secondary;
}

// --- pay, mirrored from `pay.rs` ------------------------------------------
// Kept in TS rather than round-tripped through a command because it is pure
// arithmetic over fields the item already carries. Any change here needs the
// matching change in `pay::annual_equivalent` / `pay::format`, the same way
// `skillMatch` above tracks `MIN_SKILLS_TO_SCORE`.

const HOURS_PER_YEAR = 2080;
const WEEKS_PER_YEAR = 52;
const MONTHS_PER_YEAR = 12;
const DAYS_PER_YEAR = 260;
const MIN_ANNUAL = 10_000;
const MAX_ANNUAL = 2_000_000;

/**
 * Normalises a posting's pay to what it would earn over a year, so an hourly
 * co-op rate and a salaried new-grad offer sort against each other honestly.
 * `null` for anything that did not disclose a figure — never a guess.
 */
export function payAnnualEquivalent(item: ScrapedItem): number | null {
  const { salary_min, salary_max, salary_period } = item;
  if (salary_min == null && salary_max == null) return null;
  const value =
    salary_min != null && salary_max != null
      ? (salary_min + salary_max) / 2
      : (salary_min ?? salary_max)!;
  const multiplier =
    salary_period === "hour"
      ? HOURS_PER_YEAR
      : salary_period === "day"
        ? DAYS_PER_YEAR
        : salary_period === "week"
          ? WEEKS_PER_YEAR
          : salary_period === "month"
            ? MONTHS_PER_YEAR
            : salary_period === "year"
              ? 1
              : value < 1_000
                ? HOURS_PER_YEAR
                : 1;
  const annual = value * multiplier;
  return annual >= MIN_ANNUAL && annual <= MAX_ANNUAL ? annual : null;
}

/** How a posting's pay reads on a card. `null` when nothing was disclosed. */
export function formatPay(item: ScrapedItem): string | null {
  const { salary_min, salary_max, salary_currency, salary_period } = item;
  if (salary_min == null && salary_max == null) return null;

  const unit =
    salary_period === "hour"
      ? "/hr"
      : salary_period === "day"
        ? "/day"
        : salary_period === "week"
          ? "/wk"
          : salary_period === "month"
            ? "/mo"
            : salary_period === "year"
              ? "/yr"
              : "";
  const money = (v: number) =>
    v >= 10_000 ? `${Math.round(v / 1000)}k` : Number.isInteger(v) ? `${v}` : v.toFixed(2);
  const currency = salary_currency ?? "USD";
  const symbol = currency === "GBP" ? "£" : currency === "EUR" ? "€" : "$";

  const body =
    salary_min != null && salary_max != null && Math.abs(salary_max - salary_min) > 0.001
      ? `${symbol}${money(salary_min)}–${symbol}${money(salary_max)}`
      : `${symbol}${money(salary_min ?? salary_max!)}`;
  const suffix = currency === "CAD" ? " CAD" : "";
  return `${body}${unit}${suffix}`;
}

// --- deadlines --------------------------------------------------------------

/** Whole days until a posting's own deadline. `null` when it published none. */
export function closesInDays(item: ScrapedItem): number | null {
  if (!item.closes_at) return null;
  return Math.round((new Date(item.closes_at).getTime() - Date.now()) / 86_400_000);
}

/**
 * Short deadline label for a card or the detail pane. Distinct from the event
 * countdown in `saved` — a role past its deadline is still worth reading, so
 * this says "Closed" rather than "Already passed".
 */
export function closesLabel(item: ScrapedItem): string | null {
  const days = closesInDays(item);
  if (days === null) return null;
  if (days < 0) return "Closed";
  if (days === 0) return "Closes today";
  if (days === 1) return "Closes tomorrow";
  if (days <= 21) return `Closes in ${days} days`;
  return `Closes ${new Date(item.closes_at!).toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
}

/** Close enough to call out with urgency styling rather than a plain label. */
export function closesSoon(item: ScrapedItem): boolean {
  const days = closesInDays(item);
  return days !== null && days >= 0 && days <= 3;
}

/** Extracts short attribute tags from an item's metadata. */
export function tagsFor(item: ScrapedItem): string[] {
  const tags: string[] = [];
  if (item.location) {
    tags.push(item.location.split("|")[0].trim().slice(0, 28));
    const modality = item.location_tags?.find((t) => t === "Remote" || t === "Hybrid");
    if (modality) tags.push(modality);
  } else {
    const loc = item.content_text?.match(/Location:\s*([^·|]+)/i);
    if (loc) tags.push(loc[1].trim().slice(0, 28));
  }
  if (item.discipline && item.discipline !== "General") tags.push(item.discipline);
  const term = (item.title + " " + (item.content_text ?? "")).match(
    /(Summer|Fall|Winter|Spring)\s*20\d\d/i,
  );
  if (term) tags.push(term[0]);
  return tags.slice(0, 3);
}

/**
 * Source-aware call to action, so the button says what the click will do.
 * No trailing arrow: the button renders an ArrowUpRight icon beside the label.
 */
export function ctaLabel(item: ScrapedItem): string {
  if (item.item_type === "Internship" || item.item_type === "Job") {
    if (item.source_platform === "Simplify") return "Apply via Simplify";
    if (item.source_platform === "Job Bank Canada") return "View on Job Bank";
    return "Apply on company page";
  }
  if (item.item_type === "Event") {
    if (item.source_platform === "Luma") return "Register on Luma";
    if (item.source_platform === "Devpost") return "View hackathon";
    return "View event";
  }
  const domain = domainOf(item.url);
  return domain ? `Read on ${domain}` : "Open link";
}

/** Where this item came from and what that implies about it. */
export function sourceBlurb(item: ScrapedItem): string {
  switch (item.source_platform) {
    case "Pitt CSC Repo":
      return "Community-maintained internship list from the Pitt CSC GitHub repo.";
    case "Simplify":
      return "Aggregated by Simplify — one-click applications for many of these roles.";
    case "Lassonde News":
      return "Official news from York University's Lassonde School of Engineering.";
    case "Luma":
      return "Community event listed on Luma.";
    case "Daily.dev":
      return "Trending in the daily.dev software engineering feed.";
    case "Hacker News":
    case "HN Top Links":
      return "Currently on the Hacker News front page.";
    case "Phoronix":
      return "Linux and open-source hardware coverage from Phoronix.";
    case "TLDR Tech":
      return "From today's TLDR Tech newsletter.";
    case "New Grad Positions":
      return "Full-time graduate roles, from the Simplify new-grad list.";
    case "Job Bank Canada":
      return "Posted on the Government of Canada's official Job Bank.";
    case "Devpost":
      return "Open hackathon on Devpost — check the deadline before you commit.";
    case "Lobsters":
      return "Computing-focused link aggregator, lighter on noise than HN.";
    case "Ars Technica":
      return "In-depth technology reporting from Ars Technica.";
    case "The Verge":
      return "Consumer tech and industry news from The Verge.";
    case "InfoQ":
      return "Software architecture and engineering practice from InfoQ.";
    default:
      return `Aggregated from ${item.source_platform}.`;
  }
}

/**
 * Whether the enrichment pass reached this posting, so the UI is showing what
 * the employer wrote rather than nothing at all.
 *
 * Mirrors `skills::from_posting` in Rust field for field. Every skill the app
 * shows is read off the posting's own text now — nothing is inferred from the
 * job title — so this is what separates "this posting asks for nothing we
 * recognise" from "we could not read this posting". An empty panel means
 * opposite things in those two cases.
 */
export function hasScrapedPosting(item: ScrapedItem): boolean {
  return Boolean(
    item.requirements?.trim() ||
      item.responsibilities?.trim() ||
      item.description?.trim() ||
      item.tagged_skills?.length,
  );
}
