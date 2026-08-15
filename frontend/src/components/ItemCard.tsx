"use client";

import { useState } from "react";
import { ScrapedItem, logoFor, timeAgo } from "../lib/api";
import { ArrowRightIcon, StarIcon } from "./icons";

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
  const term = (item.title + " " + (item.content_text ?? "")).match(/(Summer|Fall|Winter|Spring)\s*20\d\d/i);
  if (term) tags.push(term[0]);
  return tags.slice(0, 3);
}

export default function ItemCard({
  item,
  onOpen,
  index = 0,
}: {
  item: ScrapedItem;
  onOpen: (item: ScrapedItem) => void;
  index?: number;
}) {
  const [logoFailed, setLogoFailed] = useState(false);
  const logo = logoFor(item.url);
  const { primary, secondary } = splitTitle(item);
  const tags = tagsFor(item);

  return (
    <button
      onClick={() => onOpen(item)}
      style={{ animationDelay: `${Math.min(index, 12) * 30}ms` }}
      className="animate-fade-up group relative flex w-full flex-col gap-3 rounded-2xl border border-line bg-surface p-5 text-left transition-all duration-200 hover:-translate-y-0.5 hover:border-brand hover:"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2.5">
          {logo && !logoFailed ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={logo}
              alt=""
              width={36}
              height={36}
              loading="lazy"
              onError={() => setLogoFailed(true)}
              className="h-9 w-9 rounded-lg bg-line-soft object-contain p-1"
            />
          ) : (
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-line-soft text-sm font-bold text-subtle">
              {primary.charAt(0).toUpperCase()}
            </div>
          )}
          <span
            className={`rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider ${typeBadgeClass(item.item_type)}`}
          >
            {item.item_type}
          </span>
        </div>
        {item.relevance_score >= 10 && (
          <span className="flex shrink-0 items-center gap-1 rounded-full bg-star-soft px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide text-star">
            <StarIcon className="h-3 w-3" />
            Top match
          </span>
        )}
      </div>

      <div className="flex flex-col gap-0.5">
        <h3 className="line-clamp-2 text-lg font-bold leading-tight transition-colors group-hover:text-brand-soft-fg">
          {primary}
        </h3>
        {secondary && (
          <p className="line-clamp-1 text-sm font-medium text-muted">{secondary}</p>
        )}
        <span className="mt-0.5 text-xs font-medium text-subtle">{item.source_platform}</span>
      </div>

      {!secondary && (
        <p className="line-clamp-2 text-sm leading-relaxed text-muted">
          {item.content_text || "Open for details."}
        </p>
      )}

      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {tags.map((t) => (
            <span
              key={t}
              className="rounded-md bg-line-soft px-2 py-0.5 text-[11px] font-medium text-muted"
            >
              {t}
            </span>
          ))}
        </div>
      )}

      <div className="mt-auto flex items-center justify-between border-t border-line pt-3 text-[11px] font-medium text-subtle">
        <span>{timeAgo(item.timestamp)}</span>
        <span className="flex items-center gap-1 text-brand-soft-fg opacity-0 transition-opacity group-hover:opacity-100">
          View details
          <ArrowRightIcon className="h-3 w-3" />
        </span>
      </div>
    </button>
  );
}
