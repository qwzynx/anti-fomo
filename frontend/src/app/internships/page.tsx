"use client";

import { useEffect, useMemo, useState } from "react";
import Header from "../../components/Header";
import ItemCard, { splitTitle } from "../../components/ItemCard";
import ItemModal from "../../components/ItemModal";
import { API_BASE, ScrapedItem } from "../../lib/api";

const DISCIPLINES = ["All", "Software Engineering", "General"];
const SPECIALTIES = ["Frontend", "Backend", "Full-Stack", "DevOps", "AI/ML", "Embedded", "Data", "Product", "Security"];
const MODALITIES = ["All", "Remote", "Hybrid", "On-site"] as const;
const LOCATIONS = ["Canada", "USA", "Global / Multi-region", "Toronto", "Vancouver", "Waterloo", "San Francisco", "New York", "Seattle", "London"];
const FRESHNESS = [
  { label: "Any time", hours: Infinity },
  { label: "Last 24 hours", hours: 24 },
  { label: "Past week", hours: 24 * 7 },
  { label: "Past month", hours: 24 * 30 },
] as const;
const SORTS = ["Relevance", "Newest first", "Company name"] as const;

export default function InternshipsPage() {
  const [items, setItems] = useState<ScrapedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<ScrapedItem | null>(null);

  const [search, setSearch] = useState("");
  const [discipline, setDiscipline] = useState("All");
  const [sources, setSources] = useState<string[]>([]);
  const [specialties, setSpecialties] = useState<string[]>([]);
  const [modality, setModality] = useState<(typeof MODALITIES)[number]>("All");
  const [locations, setLocations] = useState<string[]>([]);
  const [freshness, setFreshness] = useState<(typeof FRESHNESS)[number]["label"]>("Any time");
  const [sort, setSort] = useState<(typeof SORTS)[number]>("Relevance");

  useEffect(() => {
    fetch(`${API_BASE}/api/internships`)
      .then((r) => r.json())
      .then(setItems)
      .catch((e) => console.error("Failed to fetch internships:", e))
      .finally(() => setLoading(false));
  }, []);

  const allSources = useMemo(
    () => Array.from(new Set(items.map((i) => i.source_platform))).sort(),
    [items]
  );

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    const maxAge = FRESHNESS.find((f) => f.label === freshness)?.hours ?? Infinity;
    const cutoff = Date.now() - maxAge * 3600 * 1000;

    const result = items.filter((item) => {
      const haystack = `${item.title} ${item.content_text ?? ""} ${item.location ?? ""}`.toLowerCase();
      const locTags = item.location_tags ?? [];
      if (q && !haystack.includes(q)) return false;
      if (discipline !== "All" && item.discipline !== discipline) return false;
      if (sources.length > 0 && !sources.includes(item.source_platform)) return false;
      if (specialties.length > 0 && !specialties.some((s) => haystack.includes(s.toLowerCase().replace("full-stack", "full stack")) || haystack.includes(s.toLowerCase()))) return false;
      if (modality !== "All" && !locTags.includes(modality)) return false;
      if (locations.length > 0 && !locations.some((l) => locTags.includes(l))) return false;
      if (maxAge !== Infinity && new Date(item.timestamp).getTime() < cutoff) return false;
      return true;
    });

    switch (sort) {
      case "Newest first":
        return result.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      case "Company name":
        return result.sort((a, b) => splitTitle(a).primary.localeCompare(splitTitle(b).primary));
      default:
        return result.sort((a, b) => b.relevance_score - a.relevance_score);
    }
  }, [items, search, discipline, sources, specialties, modality, locations, freshness, sort]);

  function toggle(list: string[], setList: (v: string[]) => void, value: string) {
    setList(list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);
  }

  const activeFilters =
    (discipline !== "All" ? 1 : 0) + sources.length + specialties.length +
    (modality !== "All" ? 1 : 0) + locations.length + (freshness !== "Any time" ? 1 : 0);

  return (
    <div className="flex flex-col min-h-screen bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <Header active="internships" />

      <main className="flex-1 mx-auto w-full max-w-6xl py-10 px-6">
        <div className="mb-8">
          <h2 className="text-3xl font-bold mb-1">💼 Internship Hub</h2>
          <p className="text-zinc-500 dark:text-zinc-400 text-sm">
            {loading ? "Scanning sources…" : `${filtered.length} of ${items.length} open roles from ${allSources.length} sources`}
            {activeFilters > 0 && ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""} active`}
          </p>
        </div>

        {/* Filter panel */}
        <div className="mb-8 flex flex-col gap-4 rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900">
          <div className="flex flex-col gap-3 md:flex-row">
            <input
              type="text"
              placeholder="Search roles, companies, locations…"
              className="flex-1 px-4 py-2.5 bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-xl focus:outline-none focus:ring-2 focus:ring-indigo-500"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <select
              value={discipline}
              onChange={(e) => setDiscipline(e.target.value)}
              className="bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-xl px-3 py-2.5 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
            >
              {DISCIPLINES.map((d) => (
                <option key={d} value={d}>{d === "All" ? "All disciplines" : d}</option>
              ))}
            </select>
            <select
              value={freshness}
              onChange={(e) => setFreshness(e.target.value as typeof freshness)}
              className="bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-xl px-3 py-2.5 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
            >
              {FRESHNESS.map((f) => (
                <option key={f.label} value={f.label}>{f.label}</option>
              ))}
            </select>
            <select
              value={sort}
              onChange={(e) => setSort(e.target.value as typeof sort)}
              className="bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded-xl px-3 py-2.5 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500"
            >
              {SORTS.map((s) => (
                <option key={s} value={s}>Sort: {s}</option>
              ))}
            </select>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400 mr-1">Specialty</span>
            {SPECIALTIES.map((s) => (
              <button
                key={s}
                onClick={() => toggle(specialties, setSpecialties, s)}
                className={`rounded-full px-3 py-1 text-xs font-semibold transition-colors ${
                  specialties.includes(s)
                    ? "bg-indigo-600 text-white"
                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700"
                }`}
              >
                {s}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400 mr-1">Work mode</span>
            {MODALITIES.map((m) => (
              <button
                key={m}
                onClick={() => setModality(m)}
                className={`rounded-full px-3 py-1 text-xs font-semibold transition-colors ${
                  modality === m
                    ? "bg-emerald-600 text-white"
                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700"
                }`}
              >
                {m === "Remote" ? "🌍 Remote" : m}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400 mr-1">Location</span>
            {LOCATIONS.map((l) => (
              <button
                key={l}
                onClick={() => toggle(locations, setLocations, l)}
                className={`rounded-full px-3 py-1 text-xs font-semibold transition-colors ${
                  locations.includes(l)
                    ? "bg-indigo-600 text-white"
                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700"
                }`}
              >
                {l}
              </button>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-semibold uppercase tracking-wide text-zinc-400 mr-1">Source</span>
            {allSources.map((s) => (
              <button
                key={s}
                onClick={() => toggle(sources, setSources, s)}
                className={`rounded-full px-3 py-1 text-xs font-semibold transition-colors ${
                  sources.includes(s)
                    ? "bg-indigo-600 text-white"
                    : "bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700"
                }`}
              >
                {s}
              </button>
            ))}
            {activeFilters > 0 && (
              <button
                onClick={() => { setDiscipline("All"); setSources([]); setSpecialties([]); setModality("All"); setLocations([]); setFreshness("Any time"); }}
                className="ml-auto text-xs font-semibold text-indigo-600 hover:underline dark:text-indigo-400"
              >
                Clear filters
              </button>
            )}
          </div>
        </div>

        {loading ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
            {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((i) => (
              <div key={i} className="h-52 w-full animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800" />
            ))}
          </div>
        ) : filtered.length > 0 ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
            {filtered.map((item, idx) => (
              <ItemCard key={`${item.url}-${idx}`} item={item} index={idx} onOpen={setSelected} />
            ))}
          </div>
        ) : (
          <div className="text-center py-20 bg-white dark:bg-zinc-900 rounded-3xl border border-dashed border-zinc-300 dark:border-zinc-700">
            <p className="text-zinc-500 dark:text-zinc-400">No roles match your filters.</p>
            <button
              onClick={() => { setSearch(""); setDiscipline("All"); setSources([]); setSpecialties([]); setModality("All"); setLocations([]); setFreshness("Any time"); }}
              className="mt-4 text-indigo-600 font-semibold text-sm hover:underline"
            >
              Clear everything
            </button>
          </div>
        )}
      </main>

      <ItemModal item={selected} onClose={() => setSelected(null)} />
    </div>
  );
}
