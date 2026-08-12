import type { ScrapedItem } from "./api";

export function typeBadgeClass(type: ScrapedItem["item_type"]): string {
  switch (type) {
    case "Internship":
    case "Job":
      return "bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300";
    case "Event":
      return "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300";
    default:
      return "bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300";
  }
}

/** Splits "Role at Company" style titles for a stronger visual hierarchy. */
export function splitTitle(item: ScrapedItem): { primary: string; secondary: string | null } {
  if (item.item_type === "Internship" || item.item_type === "Job") {
    const idx = item.title.lastIndexOf(" at ");
    if (idx > 0) {
      return {
        primary: item.title.slice(idx + 4),
        secondary: item.title.slice(0, idx),
      };
    }
  }
  return { primary: item.title, secondary: null };
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

/** Source-aware call to action, so the button says what the click will do. */
export function ctaLabel(item: ScrapedItem): string {
  if (item.item_type === "Internship" || item.item_type === "Job") {
    return item.source_platform === "Simplify" ? "Apply via Simplify ↗" : "Apply on Company Page ↗";
  }
  if (item.item_type === "Event") {
    return item.source_platform === "Luma" ? "Register on Luma ↗" : "View Event ↗";
  }
  const domain = item.url ? new URL(item.url).hostname.replace(/^www\./, "") : "";
  return domain ? `Read on ${domain} ↗` : "Open link ↗";
}

/** Where this item came from and what that implies about it. */
export function sourceBlurb(item: ScrapedItem): string {
  switch (item.source_platform) {
    case "Pitt CSC Repo":
      return "Community-maintained internship list from the Pitt CSC GitHub repo.";
    case "Simplify":
      return "Aggregated by Simplify — one-click applications for many of these roles.";
    case "Levels.fyi":
      return "Levels.fyi internship tracker, including reported compensation.";
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
    default:
      return `Aggregated from ${item.source_platform}.`;
  }
}
