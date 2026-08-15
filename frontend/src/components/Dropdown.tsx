"use client";

import { useEffect, useId, useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon } from "./icons";

/**
 * An accessible listbox that replaces the native <select>.
 *
 * A native select only lets you style the *closed* control — the option popup
 * is drawn by the OS, so it arrives with square corners, a system-blue
 * highlight and its own font no matter what the page looks like. This renders
 * the list itself, and reimplements the parts of the native control that
 * matter: it follows the WAI-ARIA combobox pattern, keeps DOM focus on the
 * trigger and tracks the highlighted row with aria-activedescendant.
 *
 * Keyboard: Up/Down move, Home/End jump, Enter/Space select, Escape closes,
 * Tab closes and moves on, and typing letters jumps to a matching option.
 */

export type Option = { value: string; label: string };

export function Dropdown({
  value,
  onChange,
  options,
  icon,
  label,
  className = "",
}: {
  value: string;
  onChange: (value: string) => void;
  options: Option[];
  icon?: React.ReactNode;
  /** Accessible name. Not rendered — the selected value is the visible text. */
  label: string;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const [dropUp, setDropUp] = useState(false);

  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const typeahead = useRef({ query: "", at: 0 });

  const baseId = useId();
  const listId = `${baseId}-list`;
  const optionId = (i: number) => `${baseId}-opt-${i}`;

  const selectedIndex = Math.max(0, options.findIndex((o) => o.value === value));
  const current = options[selectedIndex];

  // Close when the pointer goes down anywhere else on the page.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", onDown);
    return () => window.removeEventListener("pointerdown", onDown);
  }, [open]);

  // Keep the highlighted row in view when arrowing through a long list.
  useEffect(() => {
    if (!open) return;
    listRef.current
      ?.querySelector(`#${CSS.escape(optionId(activeIndex))}`)
      ?.scrollIntoView({ block: "nearest" });
  });

  function openList(startAt = selectedIndex) {
    // Measured here rather than in an effect: a menu near the bottom of the
    // window must open upwards instead of running off-screen.
    const rect = triggerRef.current?.getBoundingClientRect();
    if (rect) {
      const estimated = Math.min(options.length * 36 + 8, 264);
      setDropUp(rect.bottom + estimated > window.innerHeight && rect.top > estimated);
    }
    setActiveIndex(startAt);
    setOpen(true);
  }

  function commit(index: number) {
    const opt = options[index];
    if (opt) onChange(opt.value);
    setOpen(false);
    triggerRef.current?.focus();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const last = options.length - 1;

    if (!open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        openList();
      }
      return;
    }

    switch (e.key) {
      case "Escape":
        e.preventDefault();
        setOpen(false);
        triggerRef.current?.focus();
        break;
      case "Tab":
        setOpen(false);
        break;
      case "ArrowDown":
        e.preventDefault();
        setActiveIndex((i) => (i >= last ? 0 : i + 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setActiveIndex((i) => (i <= 0 ? last : i - 1));
        break;
      case "Home":
        e.preventDefault();
        setActiveIndex(0);
        break;
      case "End":
        e.preventDefault();
        setActiveIndex(last);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        commit(activeIndex);
        break;
      default: {
        if (e.key.length !== 1) return;
        // Typeahead: repeated presses of one letter cycle matches, otherwise
        // the query accumulates. Matches the native select's behaviour.
        const now = Date.now();
        const t = typeahead.current;
        t.query = now - t.at > 800 ? e.key : t.query + e.key;
        t.at = now;
        const q = t.query.toLowerCase();
        const found = options.findIndex((o) => o.label.toLowerCase().startsWith(q));
        if (found >= 0) setActiveIndex(found);
      }
    }
  }

  return (
    <div ref={rootRef} className={`relative ${className}`}>
      <button
        ref={triggerRef}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open ? optionId(activeIndex) : undefined}
        aria-label={label}
        onClick={() => (open ? setOpen(false) : openList())}
        onKeyDown={onKeyDown}
        className={`flex w-full cursor-pointer items-center gap-2 rounded-xl border bg-card py-2.5 pl-3.5 pr-3 text-sm font-medium text-ink transition-colors ${
          open ? "border-brand" : "border-line hover:border-line-strong"
        }`}
      >
        {icon && <span className="shrink-0 text-ink-3">{icon}</span>}
        <span className="flex-1 truncate text-left">{current?.label ?? ""}</span>
        <ChevronDownIcon
          className={`h-4 w-4 shrink-0 text-ink-3 transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open && (
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          aria-label={label}
          className={`animate-fade-up absolute z-30 max-h-64 w-full min-w-max overflow-y-auto rounded-xl border border-line bg-card p-1 shadow-[var(--shadow-modal)] ${
            dropUp ? "bottom-full mb-1" : "top-full mt-1"
          }`}
        >
          {options.map((opt, i) => {
            const isSelected = opt.value === value;
            return (
              <li
                key={opt.value}
                id={optionId(i)}
                role="option"
                aria-selected={isSelected}
                onPointerEnter={() => setActiveIndex(i)}
                onClick={() => commit(i)}
                className={`flex cursor-pointer items-center gap-2 rounded-lg px-3 py-1.5 text-sm transition-colors ${
                  i === activeIndex ? "bg-brand-soft text-brand-ink" : "text-ink-2"
                } ${isSelected ? "font-semibold" : ""}`}
              >
                <span className="flex-1 whitespace-nowrap">{opt.label}</span>
                {isSelected && <CheckIcon className="h-3.5 w-3.5 shrink-0" />}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
