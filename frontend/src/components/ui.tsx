"use client";

import { ChevronDownIcon, SearchIcon } from "./icons";

/**
 * Shared form primitives.
 *
 * `Select` keeps a real <select> underneath — the popup, keyboard behaviour and
 * mobile wheel all come free that way — and only restyles the closed control so
 * it stops looking like an OS default next to the rounded surfaces around it.
 */
export function Select({
  value,
  onChange,
  icon,
  label,
  children,
  className = "",
}: {
  value: string;
  onChange: (value: string) => void;
  icon?: React.ReactNode;
  /** Accessible name; not shown, the chosen value is the visible text. */
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={`relative ${className}`}>
      {icon && (
        <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-ink-3">
          {icon}
        </span>
      )}
      <select
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`w-full cursor-pointer appearance-none rounded-xl border border-line bg-card py-2.5 text-sm font-medium text-ink transition-colors hover:border-line-strong focus:outline-none ${
          icon ? "pl-9" : "pl-3.5"
        } pr-9`}
      >
        {children}
      </select>
      <ChevronDownIcon className="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-3" />
    </div>
  );
}

export function SearchField({
  value,
  onChange,
  placeholder,
  className = "",
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  className?: string;
}) {
  return (
    <div className={`relative ${className}`}>
      <SearchIcon className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-3" />
      <input
        type="search"
        aria-label={placeholder}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-xl border border-line bg-card py-2.5 pl-10 pr-4 text-sm text-ink transition-colors placeholder:text-ink-3 hover:border-line-strong focus:outline-none"
      />
    </div>
  );
}

/** Pill toggle used by the filter facets. */
export function Chip({
  active,
  onClick,
  children,
  tone = "brand",
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  tone?: "brand" | "job";
}) {
  const activeClass =
    tone === "job"
      ? "bg-job-ink text-page border-job-ink"
      : "bg-brand text-on-brand border-brand";

  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={`rounded-full border px-3 py-1 text-xs font-semibold transition-colors ${
        active
          ? activeClass
          : "border-line bg-sunken text-ink-2 hover:border-line-strong hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

/** Label that introduces a row of chips. */
export function FacetLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="mr-1 text-[11px] font-semibold uppercase tracking-wider text-ink-3">
      {children}
    </span>
  );
}
