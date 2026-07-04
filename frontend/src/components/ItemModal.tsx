"use client";

import { useEffect } from "react";
import { ScrapedItem, domainOf, logoFor } from "../lib/api";
import { splitTitle, tagsFor, typeBadgeClass } from "./ItemCard";

function ctaLabel(item: ScrapedItem): string {
  const domain = domainOf(item.url);
  switch (item.item_type) {
    case "Internship":
    case "Job":
      return item.source_platform === "Simplify"
        ? "Apply via Simplify ↗"
        : "Apply on Company Page ↗";
    case "Event":
      return domain.includes("lu.ma") || item.source_platform === "Luma"
        ? "Register on Luma ↗"
        : "View Event ↗";
    default:
      return `Read on ${domain || item.source_platform} ↗`;
  }
}

function sourceBlurb(item: ScrapedItem): string {
  switch (item.source_platform) {
    case "Pitt CSC Repo":
      return "Aggregated from the Pitt CSC Summer Internships GitHub repo";
    case "Simplify":
      return "Aggregated from the SimplifyJobs Summer Internships GitHub repo";
    case "Levels.fyi":
      return "Aggregated from the Levels.fyi internship dataset";
    case "Lassonde News":
      return "From the Lassonde School of Engineering newsroom";
    case "Luma":
      return "From Luma's Toronto events calendar";
    default:
      return `Aggregated from ${item.source_platform}`;
  }
}

export default function ItemModal({
  item,
  onClose,
}: {
  item: ScrapedItem | null;
  onClose: () => void;
}) {
  useEffect(() => {
    if (!item) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
    };
  }, [item, onClose]);

  if (!item) return null;

  const logo = logoFor(item.url);
  const { primary, secondary } = splitTitle(item);
  const tags = tagsFor(item);
  const cta = ctaLabel(item);

  return (
    <div
      className="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="animate-fade-up flex max-h-[88vh] w-full max-w-xl flex-col overflow-hidden rounded-t-3xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-800 dark:bg-zinc-900 sm:rounded-3xl"
      >
        <div className="flex items-start justify-between gap-4 border-b border-zinc-100 p-6 dark:border-zinc-800">
          <div className="flex items-center gap-3">
            {logo ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={logo}
                alt=""
                width={48}
                height={48}
                className="h-12 w-12 rounded-xl bg-zinc-100 object-contain p-1.5 dark:bg-zinc-800"
              />
            ) : null}
            <div>
              <span
                className={`text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded-full ${typeBadgeClass(item.item_type)}`}
              >
                {item.item_type}
              </span>
              <h2 className="mt-1.5 text-xl font-bold leading-tight">{primary}</h2>
              {secondary && (
                <p className="text-sm font-medium text-zinc-600 dark:text-zinc-400">{secondary}</p>
              )}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <a
              href={item.url}
              target="_blank"
              rel="noopener noreferrer"
              className="hidden rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-indigo-500 sm:block"
            >
              {cta}
            </a>
            <button
              onClick={onClose}
              aria-label="Close"
              className="flex h-8 w-8 items-center justify-center rounded-full text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
            >
              ✕
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <p className="mb-4 text-xs font-medium uppercase tracking-wide text-zinc-400">
            {sourceBlurb(item)} · {new Date(item.timestamp).toLocaleString()}
          </p>

          <p className="whitespace-pre-line text-[15px] leading-relaxed text-zinc-700 dark:text-zinc-300">
            {item.content_text || "No further details were scraped for this item — open the source for the full picture."}
          </p>

          {(tags.length > 0 || item.discipline) && (
            <div className="mt-6 flex flex-wrap gap-2">
              {[item.discipline, ...tags, item.source_platform]
                .filter((t, i, arr) => t && arr.indexOf(t) === i)
                .map((t) => (
                  <span
                    key={t}
                    className="rounded-full bg-zinc-100 px-3 py-1 text-xs font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400"
                  >
                    {t}
                  </span>
                ))}
            </div>
          )}
        </div>

        <div className="border-t border-zinc-100 p-4 dark:border-zinc-800">
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="block w-full rounded-xl bg-indigo-600 py-3 text-center font-semibold text-white transition-all hover:bg-indigo-500 hover:shadow-lg hover:shadow-indigo-600/25"
          >
            {cta}
          </a>
        </div>
      </div>
    </div>
  );
}
