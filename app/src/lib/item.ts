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
