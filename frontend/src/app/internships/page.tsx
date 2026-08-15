"use client";

import { useEffect, useMemo, useState } from "react";
import Header from "../../components/Header";
import Footer from "../../components/Footer";
import ItemCard, { splitTitle } from "../../components/ItemCard";
import ItemModal from "../../components/ItemModal";
import { Chip, FacetLabel, SearchField } from "../../components/ui";
import { Dropdown } from "../../components/Dropdown";
import { CARD_GRID, CardSkeletonGrid, EmptyState, ErrorState } from "../../components/states";
import { BriefcaseIcon, ChevronDownIcon, ClockIcon, LayersIcon, SortIcon } from "../../components/icons";
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
  const [error, setError] = useState<string | null>(null);
  // Reference time for the freshness filter — see the note on the feed page.
  const [fetchedAt, setFetchedAt] = useState(0);
  const [selected, setSelected] = useState<ScrapedItem | null>(null);
  const [showFilters, setShowFilters] = useState(false);

  const [search, setSearch] = useState("");
  const [discipline, setDiscipline] = useState("All");
  const [sources, setSources] = useState<string[]>([]);
  const [specialties, setSpecialties] = useState<string[]>([]);
  const [modality, setModality] = useState<(typeof MODALITIES)[number]>("All");
  const [locations, setLocations] = useState<string[]>([]);
  const [freshness, setFreshness] = useState<(typeof FRESHNESS)[number]["label"]>("Any time");
  const [sort, setSort] = useState<(typeof SORTS)[number]>("Relevance");

  // Bumping this re-runs the fetch effect; see the note on the feed page.
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    const controller = new AbortController();

    (async () => {
      try {
        const res = await fetch(`${API_BASE}/api/internships`, {
          signal: controller.signal,
        });
        if (!res.ok) throw new Error(`The API responded with ${res.status}.`);
        const data = await res.json();
        if (controller.signal.aborted) return;
        setItems(data);
        setError(null);
      } catch (err) {
        if (controller.signal.aborted) return;
        setItems([]);
        setError(
          err instanceof Error ? err.message : "The internships API could not be reached."
        );
      } finally {
        if (!controller.signal.aborted) {
          setFetchedAt(Date.now());
          setLoading(false);
        }
      }
    })();

    return () => controller.abort();
  }, [reloadToken]);

  function retry() {
    setLoading(true);
    setError(null);
    setReloadToken((n) => n + 1);
  }

  const allSources = useMemo(
    () => Array.from(new Set(items.map((i) => i.source_platform))).sort(),
    [items]
  );

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    const maxAge = FRESHNESS.find((f) => f.label === freshness)?.hours ?? Infinity;
    const cutoff = fetchedAt - maxAge * 3600 * 1000;

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
  }, [items, search, discipline, sources, specialties, modality, locations, freshness, sort, fetchedAt]);

  function toggle(list: string[], setList: (v: string[]) => void, value: string) {
    setList(list.includes(value) ? list.filter((v) => v !== value) : [...list, value]);
  }

  const activeFilters =
    (discipline !== "All" ? 1 : 0) + sources.length + specialties.length +
    (modality !== "All" ? 1 : 0) + locations.length + (freshness !== "Any time" ? 1 : 0);

  function clearAll() {
    setDiscipline("All");
    setSources([]);
    setSpecialties([]);
    setModality("All");
    setLocations([]);
    setFreshness("Any time");
  }

  return (
    <div className="flex min-h-screen flex-col bg-page font-sans text-ink">
      <Header active="internships" />

      <main className="mx-auto w-full max-w-6xl flex-1 px-6 py-10">
        <div className="mb-6">
          <h2 className="mb-1 flex items-center gap-2.5 text-3xl font-bold">
            <BriefcaseIcon className="h-7 w-7 text-brand-ink" />
            Internship hub
          </h2>
          <p className="text-sm text-ink-2">
            {loading
              ? "Scanning sources…"
              : `${filtered.length} of ${items.length} open roles from ${allSources.length} source${allSources.length === 1 ? "" : "s"}`}
            {activeFilters > 0 && ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""} active`}
          </p>
        </div>

        {/* Controls. The facet chips are collapsed by default — four always-on
            rows pushed every result below the fold. */}
        <div className="mb-8 rounded-2xl border border-line bg-card p-4">
          <div className="flex flex-col gap-3 lg:flex-row">
            <SearchField
              value={search}
              onChange={setSearch}
              placeholder="Search roles, companies, locations…"
              className="flex-1"
            />
            <Dropdown
              label="Discipline"
              value={discipline}
              onChange={setDiscipline}
              icon={<LayersIcon className="h-4 w-4" />}
              options={DISCIPLINES.map((d) => ({
                value: d,
                label: d === "All" ? "All disciplines" : d,
              }))}
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
          </div>

          <div className="mt-3 flex items-center gap-3 border-t border-line pt-3">
            <button
              onClick={() => setShowFilters((v) => !v)}
              aria-expanded={showFilters}
              className="flex items-center gap-1.5 rounded-lg text-sm font-semibold text-ink-2 transition-colors hover:text-ink"
            >
              <ChevronDownIcon
                className={`h-4 w-4 transition-transform ${showFilters ? "rotate-180" : ""}`}
              />
              {showFilters ? "Hide filters" : "More filters"}
              {activeFilters > 0 && (
                <span className="ml-1 rounded-full bg-brand px-1.5 py-0.5 text-[10px] font-bold text-on-brand">
                  {activeFilters}
                </span>
              )}
            </button>
            {activeFilters > 0 && (
              <button
                onClick={clearAll}
                className="ml-auto text-xs font-semibold text-brand-ink hover:underline"
              >
                Clear filters
              </button>
            )}
          </div>

          {showFilters && (
            <div className="animate-fade-up mt-4 flex flex-col gap-4">
              <div className="flex flex-wrap items-center gap-2">
                <FacetLabel>Specialty</FacetLabel>
                {SPECIALTIES.map((s) => (
                  <Chip
                    key={s}
                    active={specialties.includes(s)}
                    onClick={() => toggle(specialties, setSpecialties, s)}
                  >
                    {s}
                  </Chip>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <FacetLabel>Work mode</FacetLabel>
                {MODALITIES.map((m) => (
                  <Chip key={m} tone="job" active={modality === m} onClick={() => setModality(m)}>
                    {m}
                  </Chip>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <FacetLabel>Location</FacetLabel>
                {LOCATIONS.map((l) => (
                  <Chip
                    key={l}
                    active={locations.includes(l)}
                    onClick={() => toggle(locations, setLocations, l)}
                  >
                    {l}
                  </Chip>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <FacetLabel>Source</FacetLabel>
                {allSources.map((s) => (
                  <Chip
                    key={s}
                    active={sources.includes(s)}
                    onClick={() => toggle(sources, setSources, s)}
                  >
                    {s}
                  </Chip>
                ))}
              </div>
            </div>
          )}
        </div>

        {loading ? (
          <CardSkeletonGrid count={9} />
        ) : error ? (
          <ErrorState message={error} onRetry={retry} />
        ) : filtered.length > 0 ? (
          <div className={CARD_GRID}>
            {filtered.map((item, idx) => (
              <ItemCard key={`${item.url}-${idx}`} item={item} index={idx} onOpen={setSelected} />
            ))}
          </div>
        ) : (
          <EmptyState
            title="No roles match those filters"
            hint="Loosen a facet or clear the search to see every open role again."
            actionLabel="Clear everything"
            onAction={() => {
              setSearch("");
              clearAll();
            }}
          />
        )}
      </main>

      <ItemModal item={selected} onClose={() => setSelected(null)} />
      <Footer />
    </div>
  );
}
