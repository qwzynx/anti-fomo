"use client";

import { useEffect, useMemo, useState } from "react";
import Header from "../components/Header";
import Footer from "../components/Footer";
import ItemCard from "../components/ItemCard";
import ItemModal from "../components/ItemModal";
import { SearchField } from "../components/ui";
import { Dropdown } from "../components/Dropdown";
import { CARD_GRID, CardSkeletonGrid, EmptyState, ErrorState } from "../components/states";
import { ClockIcon, FlameIcon, GlobeIcon, LayersIcon, NewspaperIcon, SortIcon } from "../components/icons";
import { API_BASE, ScrapedItem, timeAgo } from "../lib/api";

const NEWS_SOURCES = ["Hacker News", "Phoronix", "TLDR Tech", "HN Top Links", "Daily.dev"];
const TYPE_OPTIONS = ["All", "Internships", "Events", "Articles"] as const;
const TYPE_MAP: Record<string, string[]> = {
  Internships: ["Internship", "Job"],
  Events: ["Event"],
  Articles: ["Article"],
};
const FRESHNESS = [
  { label: "Any time", hours: Infinity },
  { label: "Last 24 hours", hours: 24 },
  { label: "Past week", hours: 24 * 7 },
  { label: "Past month", hours: 24 * 30 },
] as const;
const SORTS = ["Relevance", "Newest first"] as const;

export default function Home() {
  const [items, setItems] = useState<ScrapedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  // Reference time for the freshness filter, captured when the data lands.
  // Calling Date.now() inside the filter memo would make it impure — the same
  // inputs would produce different results on an unrelated re-render.
  const [fetchedAt, setFetchedAt] = useState(0);
  const [major, setMajor] = useState("Software Engineering");
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedDiscipline, setSelectedDiscipline] = useState("All");
  const [selected, setSelected] = useState<ScrapedItem | null>(null);
  const [itemType, setItemType] = useState<(typeof TYPE_OPTIONS)[number]>("All");
  const [sourceFilter, setSourceFilter] = useState("All");
  const [freshness, setFreshness] = useState<(typeof FRESHNESS)[number]["label"]>("Any time");
  const [sort, setSort] = useState<(typeof SORTS)[number]>("Relevance");

  // Bumping this re-runs the fetch effect; it is how "Try again" retries
  // without the effect having to call a setState-bearing callback.
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    const controller = new AbortController();

    (async () => {
      try {
        const response = await fetch(
          `${API_BASE}/api/feed?major=${encodeURIComponent(major)}`,
          { signal: controller.signal }
        );
        if (!response.ok) throw new Error(`The API responded with ${response.status}.`);
        const data = await response.json();
        if (controller.signal.aborted) return;
        setItems(data);
        setError(null);
      } catch (err) {
        // An abort means a newer request superseded this one — not a failure.
        if (controller.signal.aborted) return;
        setItems([]);
        setError(err instanceof Error ? err.message : "The feed API could not be reached.");
      } finally {
        if (!controller.signal.aborted) {
          setFetchedAt(Date.now());
          setLoading(false);
        }
      }
    })();

    return () => controller.abort();
  }, [major, reloadToken]);

  function retry() {
    setLoading(true);
    setError(null);
    setReloadToken((n) => n + 1);
  }

  const topMatch = useMemo(
    () =>
      items.find(
        (i) => (i.item_type === "Internship" || i.item_type === "Job") && i.relevance_score >= 10
      ) ?? null,
    [items]
  );

  const trending = useMemo(
    () => items.filter((i) => i.item_type === "Article" && NEWS_SOURCES.includes(i.source_platform)).slice(0, 3),
    [items]
  );

  const allSources = useMemo(
    () => Array.from(new Set(items.map((i) => i.source_platform))).sort(),
    [items]
  );

  // Derived from the data rather than hardcoded: the chip row is only worth
  // showing once the feed actually spans more than one discipline.
  const allDisciplines = useMemo(
    () => Array.from(new Set(items.map((i) => i.discipline).filter(Boolean))).sort(),
    [items]
  );

  const filteredItems = useMemo(() => {
    const maxAge = FRESHNESS.find((f) => f.label === freshness)?.hours ?? Infinity;
    const cutoff = fetchedAt - maxAge * 3600 * 1000;
    const needle = searchTerm.toLowerCase();

    const result = items.filter((item) => {
      const matchesSearch =
        item.title.toLowerCase().includes(needle) ||
        (item.content_text && item.content_text.toLowerCase().includes(needle));
      if (!matchesSearch) return false;
      if (selectedDiscipline !== "All" && item.discipline !== selectedDiscipline) return false;
      if (itemType !== "All" && !TYPE_MAP[itemType].includes(item.item_type)) return false;
      if (sourceFilter !== "All" && item.source_platform !== sourceFilter) return false;
      if (maxAge !== Infinity && new Date(item.timestamp).getTime() < cutoff) return false;
      return true;
    });

    return sort === "Newest first"
      ? [...result].sort((a, b) => b.timestamp.localeCompare(a.timestamp))
      : result; // API order is already relevance-ranked
  }, [items, searchTerm, selectedDiscipline, itemType, sourceFilter, freshness, sort, fetchedAt]);

  const filtersActive =
    itemType !== "All" || sourceFilter !== "All" || freshness !== "Any time" || sort !== "Relevance";

  function resetFilters() {
    setItemType("All");
    setSourceFilter("All");
    setFreshness("Any time");
    setSort("Relevance");
  }

  return (
    <div className="flex min-h-screen flex-col bg-page font-sans text-ink">
      <Header active="feed" />

      <main className="mx-auto w-full max-w-6xl flex-1 px-6 py-10">
        {/* Curated widgets. items-start lets each card take its natural height —
            stretching them left a large empty gap under the shorter one. */}
        <section className="mb-10 grid grid-cols-1 items-start gap-4 md:grid-cols-2">
          <div className="animate-fade-up rounded-2xl border border-star-soft bg-card p-5">
            <p className="mb-3 flex items-center gap-1.5 text-xs font-bold uppercase tracking-wider text-star-ink">
              <FlameIcon className="h-3.5 w-3.5" />
              Top match today
            </p>
            {loading ? (
              <div className="skeleton h-10 w-full rounded-lg" />
            ) : topMatch ? (
              <button onClick={() => setSelected(topMatch)} className="group w-full text-left">
                <p className="font-bold leading-snug transition-colors group-hover:text-brand-ink">
                  {topMatch.title}
                </p>
                <p className="mt-1 text-xs text-ink-3">
                  {topMatch.source_platform} · {topMatch.discipline} · {timeAgo(topMatch.timestamp)}
                </p>
              </button>
            ) : (
              <p className="text-sm text-ink-2">No strong match yet — check back soon.</p>
            )}
          </div>

          <div
            className="animate-fade-up rounded-2xl border border-line bg-card p-5"
            style={{ animationDelay: "60ms" }}
          >
            <p className="mb-3 flex items-center gap-1.5 text-xs font-bold uppercase tracking-wider text-news-ink">
              <NewspaperIcon className="h-3.5 w-3.5" />
              Trending in tech
            </p>
            {loading ? (
              <div className="flex flex-col gap-2">
                {[0, 1, 2].map((i) => (
                  <div key={i} className="skeleton h-8 w-full rounded-lg" />
                ))}
              </div>
            ) : trending.length > 0 ? (
              <ul className="flex flex-col gap-2">
                {trending.map((t, i) => (
                  <li key={i} className="text-sm leading-snug">
                    <button
                      onClick={() => setSelected(t)}
                      className="line-clamp-1 text-left font-semibold transition-colors hover:text-brand-ink"
                    >
                      {t.title}
                    </button>
                    <span className="text-xs text-ink-3">
                      {t.source_platform} · {timeAgo(t.timestamp)}
                    </span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-sm text-ink-2">No stories right now.</p>
            )}
          </div>
        </section>

        {/* Heading + search */}
        <div className="mb-5 flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
          <div>
            <h2 className="mb-1 text-3xl font-bold">Your feed</h2>
            <p className="text-sm text-ink-2">
              {loading
                ? "Scanning sources…"
                : `${filteredItems.length} result${filteredItems.length === 1 ? "" : "s"} for ${major} students`}
              {selectedDiscipline !== "All" && ` · ${selectedDiscipline}`}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <SearchField
              value={searchTerm}
              onChange={setSearchTerm}
              placeholder="Search your feed…"
              className="w-full md:w-64"
            />
            <Dropdown
              label="Major"
              value={major}
              onChange={(v) => {
                setLoading(true);
                setMajor(v);
              }}
              options={[{ value: "Software Engineering", label: "Software Engineering" }]}
            />
          </div>
        </div>

        {/* Filter bar */}
        <div className="mb-6 flex flex-wrap items-center gap-2">
          <Dropdown
            label="Item type"
            value={itemType}
            onChange={(v) => setItemType(v as typeof itemType)}
            icon={<LayersIcon className="h-4 w-4" />}
            options={TYPE_OPTIONS.map((t) => ({
              value: t,
              label: t === "All" ? "All types" : t,
            }))}
          />
          <Dropdown
            label="Source"
            value={sourceFilter}
            onChange={setSourceFilter}
            icon={<GlobeIcon className="h-4 w-4" />}
            options={[
              { value: "All", label: "All sources" },
              ...allSources.map((s) => ({ value: s, label: s })),
            ]}
          />
          <Dropdown
            label="Freshness"
            value={freshness}
            onChange={(v) => setFreshness(v as typeof freshness)}
            icon={<ClockIcon className="h-4 w-4" />}
            options={FRESHNESS.map((f) => ({ value: f.label, label: f.label }))}
          />
          <Dropdown
            label="Sort order"
            value={sort}
            onChange={(v) => setSort(v as typeof sort)}
            icon={<SortIcon className="h-4 w-4" />}
            options={SORTS.map((s) => ({ value: s, label: s }))}
          />
          {filtersActive && (
            <button
              onClick={resetFilters}
              className="rounded-lg px-2 py-1 text-xs font-semibold text-brand-ink hover:underline"
            >
              Reset
            </button>
          )}
        </div>

        {allDisciplines.length > 1 && (
          <div className="scrollbar-hide mb-8 overflow-x-auto">
            <div className="flex gap-2">
              {["All", ...allDisciplines].map((discipline) => (
                <button
                  key={discipline}
                  onClick={() => setSelectedDiscipline(discipline)}
                  aria-pressed={selectedDiscipline === discipline}
                  className={`whitespace-nowrap rounded-full border px-3.5 py-1.5 text-xs font-semibold transition-colors ${
                    selectedDiscipline === discipline
                      ? "border-brand bg-brand text-on-brand"
                      : "border-line bg-card text-ink-2 hover:border-line-strong hover:text-ink"
                  }`}
                >
                  {discipline}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Feed */}
        {loading ? (
          <CardSkeletonGrid />
        ) : error ? (
          <ErrorState message={error} onRetry={retry} />
        ) : filteredItems.length > 0 ? (
          <div className={CARD_GRID}>
            {filteredItems.map((item, idx) => (
              <ItemCard key={`${item.url}-${idx}`} item={item} index={idx} onOpen={setSelected} />
            ))}
          </div>
        ) : (
          <EmptyState
            title="Nothing matches those filters"
            hint="Try a broader search term, or clear the filters to see the whole feed."
            actionLabel="Clear search and filters"
            onAction={() => {
              setSearchTerm("");
              setSelectedDiscipline("All");
              resetFilters();
            }}
          />
        )}
      </main>

      <ItemModal item={selected} onClose={() => setSelected(null)} />
      <Footer />
    </div>
  );
}
