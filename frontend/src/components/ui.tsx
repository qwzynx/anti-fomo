"use client";

import { SearchIcon } from "./icons";

/**
 * Shared form primitives.
 *
 * The dropdown lives in Dropdown.tsx — a native <select> could not be styled
 * past its closed state, so the option list is rendered as a real listbox.
 */

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
      <SearchIcon className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-subtle" />
      <input
        type="search"
        aria-label={placeholder}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-xl border border-line bg-surface py-2.5 pl-10 pr-4 text-sm text-foreground transition-colors placeholder:text-subtle hover:border-line focus:outline-none"
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
      ? "bg-job text-background border-job"
      : "bg-brand text-brand-fg border-brand";

  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={`rounded-full border px-3 py-1 text-xs font-semibold transition-colors ${
        active
          ? activeClass
          : "border-line bg-line-soft text-muted hover:border-line hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}

/** Label that introduces a row of chips. */
export function FacetLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="mr-1 text-[11px] font-semibold uppercase tracking-wider text-subtle">
      {children}
    </span>
  );
}
