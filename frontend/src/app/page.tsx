"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import Header from "../components/Header";
import ItemCard from "../components/ItemCard";
import ItemModal from "../components/ItemModal";
import { API_BASE, EclassUpdate, ScrapedItem, api, getToken, timeAgo } from "../lib/api";

const DISCIPLINES = ["All", "Software Engineering"];
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

interface UpdatesResponse {
  fetched_at: string | null;
  updates: EclassUpdate[];
}

export default function Home() {
  const [items, setItems] = useState<ScrapedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [major, setMajor] = useState("Software Engineering");
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedDiscipline, setSelectedDiscipline] = useState("All");
  const [selected, setSelected] = useState<ScrapedItem | null>(null);
  const [deadlines, setDeadlines] = useState<EclassUpdate[] | null>(null);
  const [signedIn, setSignedIn] = useState(false);
  const [itemType, setItemType] = useState<(typeof TYPE_OPTIONS)[number]>("All");
  const [sourceFilter, setSourceFilter] = useState("All");
  const [freshness, setFreshness] = useState<(typeof FRESHNESS)[number]["label"]>("Any time");
  const [sort, setSort] = useState<(typeof SORTS)[number]>("Relevance");

  useEffect(() => {
    async function fetchFeed() {
      setLoading(true);
      try {
        const response = await fetch(`${API_BASE}/api/feed?major=${encodeURIComponent(major)}`);
        const data = await response.json();
        setItems(data);
      } catch (error) {
        console.error("Failed to fetch feed:", error);
      } finally {
        setLoading(false);
      }
    }

    fetchFeed();
  }, [major]);

  useEffect(() => {
    if (!getToken()) return;
    setSignedIn(true);
    api<UpdatesResponse>("/api/eclass/updates")
      .then((res) => {
        const soon = Date.now() + 72 * 3600 * 1000;
        setDeadlines(
          res.updates
            .filter((u) => u.kind === "deadline" && u.timestamp && new Date(u.timestamp).getTime() <= soon)
            .sort((a, b) => (a.timestamp ?? "").localeCompare(b.timestamp ?? ""))
            .slice(0, 3)
        );
      })
      .catch(() => setDeadlines(null));
  }, []);

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

  const filteredItems = useMemo(() => {
    const maxAge = FRESHNESS.find((f) => f.label === freshness)?.hours ?? Infinity;
    const cutoff = Date.now() - maxAge * 3600 * 1000;

    const result = items.filter((item) => {
      const matchesSearch =
        item.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
        (item.content_text && item.content_text.toLowerCase().includes(searchTerm.toLowerCase()));
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
  }, [items, searchTerm, selectedDiscipline, itemType, sourceFilter, freshness, sort]);

  return (
    <div className="flex flex-col min-h-screen bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <Header active="feed" />

      <main className="flex-1 mx-auto w-full max-w-6xl py-10 px-6">
        {/* Curated widgets */}
        <section className="mb-10 grid grid-cols-1 gap-4 md:grid-cols-3">
          <div className="animate-fade-up rounded-2xl border border-amber-200/60 bg-gradient-to-br from-amber-50 to-white p-5 dark:border-amber-900/40 dark:from-amber-950/30 dark:to-zinc-900">
            <p className="mb-2 text-xs font-bold uppercase tracking-wider text-amber-600 dark:text-amber-400">
              🔥 Top Match Today
            </p>
            {topMatch ? (
              <button onClick={() => setSelected(topMatch)} className="text-left group">
                <p className="font-bold leading-snug group-hover:text-indigo-600 dark:group-hover:text-indigo-400">
                  {topMatch.title}
                </p>
                <p className="mt-1 text-xs text-zinc-500">
                  {topMatch.source_platform} · {topMatch.discipline}
                </p>
              </button>
            ) : (
              <p className="text-sm text-zinc-500">{loading ? "Scanning sources…" : "No strong match yet — check back soon."}</p>
            )}
          </div>

          <div className="animate-fade-up rounded-2xl border border-rose-200/60 bg-gradient-to-br from-rose-50 to-white p-5 dark:border-rose-900/40 dark:from-rose-950/30 dark:to-zinc-900" style={{ animationDelay: "60ms" }}>
            <p className="mb-2 text-xs font-bold uppercase tracking-wider text-rose-600 dark:text-rose-400">
              ⏰ Urgent Course Deadlines
            </p>
            {deadlines && deadlines.length > 0 ? (
              <ul className="flex flex-col gap-1.5">
                {deadlines.map((d, i) => (
                  <li key={i} className="text-sm leading-snug">
                    <a href={d.url ?? "#"} target="_blank" rel="noopener noreferrer" className="font-semibold hover:text-indigo-600 dark:hover:text-indigo-400">
                      {d.title}
                    </a>
                    <span className="text-xs text-zinc-500"> — {d.timestamp ? new Date(d.timestamp).toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" }) : ""}</span>
                  </li>
                ))}
              </ul>
            ) : deadlines ? (
              <p className="text-sm text-zinc-500">Nothing due in the next 72 hours 🎉</p>
            ) : (
              <p className="text-sm text-zinc-500">
                <Link href={signedIn ? "/dashboard" : "/login"} className="font-semibold text-indigo-600 hover:underline dark:text-indigo-400">
                  {signedIn ? "Connect eClass" : "Sign in"}
                </Link>{" "}
                to see assignment deadlines here.
              </p>
            )}
          </div>

          <div className="animate-fade-up rounded-2xl border border-indigo-200/60 bg-gradient-to-br from-indigo-50 to-white p-5 dark:border-indigo-900/40 dark:from-indigo-950/30 dark:to-zinc-900" style={{ animationDelay: "120ms" }}>
            <p className="mb-2 text-xs font-bold uppercase tracking-wider text-indigo-600 dark:text-indigo-400">
              📰 Trending in Tech
            </p>
            {trending.length > 0 ? (
              <ul className="flex flex-col gap-1.5">
                {trending.map((t, i) => (
                  <li key={i} className="text-sm leading-snug">
                    <button onClick={() => setSelected(t)} className="text-left font-semibold hover:text-indigo-600 dark:hover:text-indigo-400 line-clamp-1">
                      {t.title}
                    </button>
                    <span className="text-xs text-zinc-500">{t.source_platform} · {timeAgo(t.timestamp)}</span>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-sm text-zinc-500">{loading ? "Loading stories…" : "No stories right now."}</p>
            )}
          </div>
        </section>

        {/* Controls */}
        <div className="mb-6 flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 className="text-3xl font-bold mb-1">Your Feed</h2>
            <p className="text-zinc-500 dark:text-zinc-400 text-sm">
              {filteredItems.length} results for {major} students
              {selectedDiscipline !== "All" && ` · filtered by ${selectedDiscipline}`}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <input
              type="text"
              placeholder="Search your feed…"
              className="w-full md:w-64 px-4 py-2 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl focus:outline-none focus:ring-2 focus:ring-indigo-500 transition-all"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
            <select
              value={major}
              onChange={(e) => setMajor(e.target.value)}
              className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl px-3 py-2 outline-none focus:ring-2 focus:ring-indigo-500 text-sm font-medium"
            >
              {DISCIPLINES.filter((d) => d !== "All").map((d) => (
                <option key={d} value={d}>{d}</option>
              ))}
            </select>
          </div>
        </div>

        {/* Filter bar */}
        <div className="mb-4 flex flex-wrap items-center gap-2">
          <select
            value={itemType}
            onChange={(e) => setItemType(e.target.value as typeof itemType)}
            className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
          >
            {TYPE_OPTIONS.map((t) => (
              <option key={t} value={t}>📦 {t === "All" ? "All types" : t}</option>
            ))}
          </select>
          <select
            value={sourceFilter}
            onChange={(e) => setSourceFilter(e.target.value)}
            className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
          >
            <option value="All">🌐 All sources</option>
            {allSources.map((s) => (
              <option key={s} value={s}>{s}</option>
            ))}
          </select>
          <select
            value={freshness}
            onChange={(e) => setFreshness(e.target.value as typeof freshness)}
            className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
          >
            {FRESHNESS.map((f) => (
              <option key={f.label} value={f.label}>⏱️ {f.label}</option>
            ))}
          </select>
          <select
            value={sort}
            onChange={(e) => setSort(e.target.value as typeof sort)}
            className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-xl px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
          >
            {SORTS.map((s) => (
              <option key={s} value={s}>📊 {s}</option>
            ))}
          </select>
          {(itemType !== "All" || sourceFilter !== "All" || freshness !== "Any time" || sort !== "Relevance") && (
            <button
              onClick={() => { setItemType("All"); setSourceFilter("All"); setFreshness("Any time"); setSort("Relevance"); }}
              className="text-xs font-semibold text-indigo-600 hover:underline dark:text-indigo-400"
            >
              Reset
            </button>
          )}
        </div>

        <div className="mb-8 overflow-x-auto scrollbar-hide">
          <div className="flex gap-2">
            {DISCIPLINES.map((discipline) => (
              <button
                key={discipline}
                onClick={() => setSelectedDiscipline(discipline)}
                className={`px-3.5 py-1.5 rounded-full text-xs font-semibold whitespace-nowrap transition-colors ${
                  selectedDiscipline === discipline
                    ? "bg-indigo-600 text-white"
                    : "bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400 hover:border-indigo-400"
                }`}
              >
                {discipline}
              </button>
            ))}
          </div>
        </div>

        {/* Feed */}
        {loading ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
            {[1, 2, 3, 4, 5, 6].map((i) => (
              <div key={i} className="h-52 w-full animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800" />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
            {filteredItems.map((item, idx) => (
              <ItemCard key={`${item.url}-${idx}`} item={item} index={idx} onOpen={setSelected} />
            ))}
          </div>
        )}

        {!loading && filteredItems.length === 0 && (
          <div className="text-center py-20 bg-white dark:bg-zinc-900 rounded-3xl border border-dashed border-zinc-300 dark:border-zinc-700">
            <p className="text-zinc-500 dark:text-zinc-400">No opportunities match your search or filters.</p>
            <button
              onClick={() => { setSearchTerm(""); setSelectedDiscipline("All"); }}
              className="mt-4 text-indigo-600 font-semibold text-sm hover:underline"
            >
              Clear all filters
            </button>
          </div>
        )}
      </main>

      <ItemModal item={selected} onClose={() => setSelected(null)} />

      <footer className="mt-auto border-t border-zinc-200 py-8 text-center text-sm text-zinc-500 dark:border-zinc-800">
        <p>© 2026 Anti-FOMO. Aggregated from Hacker News, Phoronix, TLDR, Lassonde, Luma, Levels.fyi, Pitt CSC & Simplify.</p>
      </footer>
    </div>
  );
}
