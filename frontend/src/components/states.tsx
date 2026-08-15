"use client";

import { AlertIcon, InboxIcon, RefreshIcon } from "./icons";

export const CARD_GRID = "grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3";

/** Placeholder cards that match the real card's shape, so the layout does not
 *  jump when results arrive. */
export function CardSkeletonGrid({ count = 6 }: { count?: number }) {
  return (
    <div className={CARD_GRID} aria-hidden="true">
      {Array.from({ length: count }, (_, i) => (
        <div
          key={i}
          className="skeleton h-52 w-full rounded-2xl border border-line"
        />
      ))}
    </div>
  );
}

/** Shown when the API could not be reached. Previously this state rendered as
 *  an empty grid, which read as "no results" rather than "backend is down". */
export function ErrorState({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  return (
    <div
      role="alert"
      className="flex flex-col items-center rounded-3xl border border-line bg-card px-6 py-16 text-center"
    >
      <span className="flex h-12 w-12 items-center justify-center rounded-full bg-danger-soft text-danger-ink">
        <AlertIcon className="h-6 w-6" />
      </span>
      <h3 className="mt-4 text-lg font-bold">Could not load the feed</h3>
      <p className="mt-1 max-w-md text-sm text-ink-2">{message}</p>
      <p className="mt-3 max-w-md text-xs text-ink-3">
        The scraper API is served separately — check that it is running on port 8000.
      </p>
      <button
        onClick={onRetry}
        className="mt-5 flex items-center gap-2 rounded-xl bg-brand px-4 py-2 text-sm font-semibold text-on-brand transition-colors hover:bg-brand-hover"
      >
        <RefreshIcon className="h-4 w-4" />
        Try again
      </button>
    </div>
  );
}

export function EmptyState({
  title,
  hint,
  actionLabel,
  onAction,
}: {
  title: string;
  hint: string;
  actionLabel: string;
  onAction: () => void;
}) {
  return (
    <div className="flex flex-col items-center rounded-3xl border border-dashed border-line-strong bg-card px-6 py-16 text-center">
      <span className="flex h-12 w-12 items-center justify-center rounded-full bg-sunken text-ink-3">
        <InboxIcon className="h-6 w-6" />
      </span>
      <h3 className="mt-4 text-lg font-bold">{title}</h3>
      <p className="mt-1 max-w-md text-sm text-ink-2">{hint}</p>
      <button
        onClick={onAction}
        className="mt-5 rounded-xl border border-line px-4 py-2 text-sm font-semibold text-ink transition-colors hover:bg-sunken"
      >
        {actionLabel}
      </button>
    </div>
  );
}
