const SOURCES = [
  "Hacker News",
  "Phoronix",
  "TLDR Tech",
  "Lassonde News",
  "Luma",
  "Levels.fyi",
  "Pitt CSC",
  "Simplify",
];

export default function Footer() {
  return (
    <footer className="mt-auto border-t border-line">
      <div className="mx-auto max-w-6xl px-6 py-8 text-center">
        <p className="text-xs font-semibold uppercase tracking-wider text-ink-3">
          Aggregated from
        </p>
        <ul className="mt-3 flex flex-wrap items-center justify-center gap-x-2 gap-y-1.5">
          {SOURCES.map((s) => (
            <li
              key={s}
              className="rounded-md bg-sunken px-2 py-0.5 text-[11px] font-medium text-ink-2"
            >
              {s}
            </li>
          ))}
        </ul>
        <p className="mt-5 text-xs text-ink-3">© 2026 Anti-FOMO</p>
      </div>
    </footer>
  );
}
