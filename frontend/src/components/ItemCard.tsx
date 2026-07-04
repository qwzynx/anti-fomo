"use client";

import { useState } from "react";
import { ScrapedItem, logoFor, timeAgo } from "../lib/api";

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

/** Extracts short attribute tags from an item's metadata text. */
export function tagsFor(item: ScrapedItem): string[] {
  const tags: string[] = [];
  if (item.discipline && item.discipline !== "General") tags.push(item.discipline);
  const loc = item.content_text?.match(/Location:\s*([^·|]+)/i);
  if (loc) tags.push(loc[1].trim().split("</br>")[0].slice(0, 28));
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
      className="animate-fade-up group relative flex w-full flex-col gap-3 rounded-2xl border border-zinc-200 bg-white p-5 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-indigo-500 hover:shadow-lg hover:shadow-indigo-600/5 dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-400"
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
              onError={() => setLogoFailed(true)}
              className="h-9 w-9 rounded-lg bg-zinc-100 object-contain p-1 dark:bg-zinc-800"
            />
          ) : (
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-zinc-100 text-sm font-bold text-zinc-500 dark:bg-zinc-800">
              {primary.charAt(0).toUpperCase()}
            </div>
          )}
          <span
            className={`text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded-full ${typeBadgeClass(item.item_type)}`}
          >
            {item.item_type}
          </span>
        </div>
        {item.relevance_score >= 10 && (
          <span className="shrink-0 rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-bold text-amber-700 dark:bg-amber-900/30 dark:text-amber-300">
            ⭐ TOP MATCH
          </span>
        )}
      </div>

      <div className="flex flex-col gap-0.5">
        <h3 className="text-lg font-bold leading-tight group-hover:text-indigo-600 dark:group-hover:text-indigo-400 line-clamp-2">
          {primary}
        </h3>
        {secondary && (
          <p className="text-sm font-medium text-zinc-600 dark:text-zinc-400 line-clamp-1">{secondary}</p>
        )}
        <span className="text-xs text-zinc-400 font-medium mt-0.5">{item.source_platform}</span>
      </div>

      {!secondary && (
        <p className="line-clamp-2 text-sm leading-relaxed text-zinc-600 dark:text-zinc-400">
          {item.content_text || "Open for details."}
        </p>
      )}

      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {tags.map((t) => (
            <span
              key={t}
              className="rounded-md bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400"
            >
              {t}
            </span>
          ))}
        </div>
      )}

      <div className="mt-auto flex items-center justify-between border-t border-zinc-100 pt-3 text-[11px] font-medium text-zinc-400 dark:border-zinc-800">
        <span>{timeAgo(item.timestamp)}</span>
        <span className="text-indigo-500 opacity-0 transition-opacity group-hover:opacity-100">
          View details →
        </span>
      </div>
    </button>
  );
}
