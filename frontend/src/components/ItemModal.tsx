"use client";

import { useCallback, useEffect, useId, useRef } from "react";
import { ScrapedItem, domainOf, logoFor } from "../lib/api";
import { splitTitle, tagsFor, typeBadgeClass } from "./ItemCard";
import { CloseIcon, ExternalLinkIcon, MapPinIcon } from "./icons";

function ctaLabel(item: ScrapedItem): string {
  const domain = domainOf(item.url);
  switch (item.item_type) {
    case "Internship":
    case "Job":
      return item.source_platform === "Simplify"
        ? "Apply via Simplify"
        : "Apply on company page";
    case "Event":
      return domain.includes("lu.ma") || item.source_platform === "Luma"
        ? "Register on Luma"
        : "View event";
    default:
      return `Read on ${domain || item.source_platform}`;
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

const FOCUSABLE =
  'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])';

export default function ItemModal({
  item,
  onClose,
}: {
  item: ScrapedItem | null;
  onClose: () => void;
}) {
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  // True only when the press *started* on the backdrop. Without this a drag
  // that begins inside the panel and releases outside it counts as a backdrop
  // click, so selecting text or slightly missing a button closes the dialog.
  const pressedBackdrop = useRef(false);
  const titleId = useId();

  useEffect(() => {
    if (!item) return;

    restoreFocusRef.current = document.activeElement as HTMLElement | null;
    panelRef.current?.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key !== "Tab" || !panelRef.current) return;

      // Keep Tab inside the dialog while it is open.
      const focusable = Array.from(
        panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)
      ).filter((el) => el.offsetParent !== null);
      if (focusable.length === 0) return;

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";

    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
      restoreFocusRef.current?.focus?.();
    };
  }, [item, onClose]);

  const onBackdropPointerDown = useCallback((e: React.PointerEvent) => {
    pressedBackdrop.current = e.target === e.currentTarget;
  }, []);

  const onBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget && pressedBackdrop.current) onClose();
      pressedBackdrop.current = false;
    },
    [onClose]
  );

  if (!item) return null;

  const logo = logoFor(item.url);
  const { primary, secondary } = splitTitle(item);
  const tags = tagsFor(item);
  const cta = ctaLabel(item);

  return (
    <div
      className="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
      onPointerDown={onBackdropPointerDown}
      onClick={onBackdropClick}
    >
      <div
        ref={panelRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="animate-fade-up flex max-h-[88vh] w-full max-w-xl flex-col overflow-hidden rounded-t-3xl border border-line bg-card shadow-[var(--shadow-modal)] focus:outline-none sm:rounded-3xl"
      >
        <div className="flex items-start justify-between gap-4 border-b border-line p-6">
          <div className="flex items-center gap-3">
            {logo ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={logo}
                alt=""
                width={48}
                height={48}
                className="h-12 w-12 rounded-xl bg-sunken object-contain p-1.5"
              />
            ) : null}
            <div>
              <span
                className={`rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider ${typeBadgeClass(item.item_type)}`}
              >
                {item.item_type}
              </span>
              <h2 id={titleId} className="mt-1.5 text-xl font-bold leading-tight">
                {primary}
              </h2>
              {secondary && (
                <p className="text-sm font-medium text-ink-2">{secondary}</p>
              )}
            </div>
          </div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-ink-3 transition-colors hover:bg-sunken hover:text-ink"
          >
            <CloseIcon className="h-4 w-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          <p className="mb-4 text-xs font-medium uppercase tracking-wide text-ink-3">
            {sourceBlurb(item)} · {new Date(item.timestamp).toLocaleString()}
          </p>

          {item.location && (
            <div className="mb-4 rounded-xl bg-sunken p-4">
              <p className="mb-1 flex items-center gap-1.5 text-xs font-bold uppercase tracking-wide text-ink-3">
                <MapPinIcon className="h-3.5 w-3.5" />
                Locations
              </p>
              <p className="text-sm text-ink-2">
                {item.location.split("|").map((l) => l.trim()).filter(Boolean).join(" · ")}
              </p>
              {item.location_tags && item.location_tags.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {item.location_tags.map((t) => (
                    <span
                      key={t}
                      className="rounded-md bg-card px-2 py-0.5 text-[11px] font-semibold text-ink-2"
                    >
                      {t}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          <p className="whitespace-pre-line text-[15px] leading-relaxed text-ink-2">
            {item.content_text ||
              "No further details were scraped for this item — open the source for the full picture."}
          </p>

          {(tags.length > 0 || item.discipline) && (
            <div className="mt-6 flex flex-wrap gap-2">
              {[item.discipline, ...tags, item.source_platform]
                .filter((t, i, arr) => t && arr.indexOf(t) === i)
                .map((t) => (
                  <span
                    key={t}
                    className="rounded-full bg-sunken px-3 py-1 text-xs font-medium text-ink-2"
                  >
                    {t}
                  </span>
                ))}
            </div>
          )}
        </div>

        <div className="border-t border-line p-4">
          <a
            href={item.url}
            target="_blank"
            rel="noopener noreferrer"
            className="flex w-full items-center justify-center gap-2 rounded-xl bg-brand py-3 text-center font-semibold text-on-brand transition-all hover:bg-brand-hover hover:shadow-[var(--shadow-lift)]"
          >
            {cta}
            <ExternalLinkIcon className="h-4 w-4" />
          </a>
          <p className="mt-2 text-center text-[11px] text-ink-3">Opens in a new tab</p>
        </div>
      </div>
    </div>
  );
}
