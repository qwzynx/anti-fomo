<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import InfiniteScroll from "$lib/components/InfiniteScroll.svelte";
  import ItemDetail from "$lib/components/ItemDetail.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import RowSkeleton from "$lib/components/RowSkeleton.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Briefcase, ChevronDown, Filter, Search, SlidersHorizontal, X } from "$lib/icons";
  import {
    FRESHNESS,
    HUB_DISCIPLINES,
    HUB_SORTS,
    LOCATIONS,
    MODALITIES,
    SPECIALTIES,
    freshnessCutoff,
    matchesSpecialty,
    toggle,
    type FreshnessLabel,
    type HubSort,
    type Modality,
  } from "$lib/filters";
  import { skillMatch } from "$lib/skills";

  // The job-board layout: a scrolling result list beside a detail pane that
  // stays put, which is what every board this app aggregates from converges on.
  // Below lg there is no room for two columns, so the list goes full width and
  // the detail opens as a modal instead.
  //
  // This is also the app's only filter surface now. It used to have three — a
  // row of selects on the feed, a sheet on phones and an expandable facet panel
  // here — which meant the same question was asked three different ways and
  // answered independently. Everything is a chip: what is on is filled and
  // carries a cross, what is off is an outline, and nothing hides in a drawer
  // you have to open to find out what you already chose.

  /** The coverage a "good match" chip demands. Matches the ranking's own bar. */
  const GOOD_MATCH = 0.6;

  let selected = $state<ScrapedItem | null>(null);
  let moreOpen = $state(false);

  let discipline = $state("All");
  let sources = $state<string[]>([]);
  let specialties = $state<string[]>([]);
  let modality = $state<Modality>("All");
  let locations = $state<string[]>([]);
  let freshness = $state<FreshnessLabel>("Any time");
  let goodMatchOnly = $state(false);
  let sort = $state<HubSort>("Relevance");

  const PAGE = 40;
  let shown = $state(PAGE);

  /** True once there is room for two columns. Drives which of the detail pane
   *  and the modal is mounted — never both, or the modal's scroll lock would
   *  freeze the page behind a pane that is already visible. */
  let wide = $state(false);

  $effect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const sync = () => (wide = mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  });

  const items = $derived(feed.internships);
  const allSources = $derived([...new Set(items.map((i) => i.source_platform))].sort());

  const filtered = $derived.by(() => {
    const cutoff = freshnessCutoff(freshness);

    const result = items.filter((item) => {
      const haystack =
        `${item.title} ${item.content_text ?? ""} ${item.location ?? ""}`.toLowerCase();
      const locTags = item.location_tags ?? [];

      if (discipline !== "All" && item.discipline !== discipline) return false;
      if (sources.length > 0 && !sources.includes(item.source_platform)) return false;
      if (specialties.length > 0 && !specialties.some((s) => matchesSpecialty(haystack, s)))
        return false;
      if (modality !== "All" && !locTags.includes(modality)) return false;
      if (locations.length > 0 && !locations.some((l) => locTags.includes(l))) return false;
      if (cutoff !== -Infinity && new Date(item.timestamp).getTime() < cutoff) return false;
      if (goodMatchOnly) {
        const m = skillMatch(item);
        if (!m || m.have / m.total < GOOD_MATCH) return false;
      }
      return true;
    });

    switch (sort) {
      case "Newest first":
        return [...result].sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      case "Company name":
        return [...result].sort((a, b) => a.title.localeCompare(b.title));
      default:
        return [...result].sort((a, b) => (b.relevance_score ?? 0) - (a.relevance_score ?? 0));
    }
  });

  // Send the list back to the top when the filters change it, but leave paging
  // alone when a background refresh lands on the same filter.
  const signature = $derived(`${filtered.length}|${filtered[0]?.url ?? ""}`);
  $effect(() => {
    void signature;
    shown = PAGE;
  });

  const visible = $derived(filtered.slice(0, shown));

  /**
   * At desktop widths the pane opens on the top-ranked role rather than on an
   * invitation to click something — every board this app aggregates from does
   * the same, and the alternative spends two thirds of the window telling you
   * to use the other third. Re-picks when a filter leaves the selection behind.
   */
  $effect(() => {
    if (!wide) return;
    if (!selected || !filtered.some((i) => i.url === selected?.url)) {
      selected = filtered[0] ?? null;
    }
  });

  type Chip = { label: string; active: boolean; onToggle: () => void };

  /**
   * The chips on the bar itself: everything currently on, then a short list of
   * the ones worth reaching for. The rest live behind "More" — but nothing that
   * is *on* is ever hidden there, which is the whole point of chips over a
   * drawer.
   */
  const quickChips = $derived.by(() => {
    const chips: Chip[] = [];

    for (const m of MODALITIES) {
      if (m === "All") continue;
      if (m === "Remote" || modality === m)
        chips.push({ label: m, active: modality === m, onToggle: () => (modality = modality === m ? "All" : m) });
    }

    const week = FRESHNESS[2].label;
    for (const f of FRESHNESS) {
      if (f.label === "Any time") continue;
      if (f.label === week || freshness === f.label)
        chips.push({
          label: f.label === week ? "Posted this week" : f.label,
          active: freshness === f.label,
          onToggle: () => (freshness = freshness === f.label ? "Any time" : f.label),
        });
    }

    for (const l of LOCATIONS) {
      if (l === "Toronto" || locations.includes(l))
        chips.push({
          label: l,
          active: locations.includes(l),
          onToggle: () => (locations = toggle(locations, l)),
        });
    }

    for (const s of SPECIALTIES) {
      if (s === "Backend" || specialties.includes(s))
        chips.push({
          label: s,
          active: specialties.includes(s),
          onToggle: () => (specialties = toggle(specialties, s)),
        });
    }

    for (const s of sources) {
      chips.push({ label: s, active: true, onToggle: () => (sources = toggle(sources, s)) });
    }

    if (discipline !== "All") {
      chips.push({ label: discipline, active: true, onToggle: () => (discipline = "All") });
    }

    if (feed.skills.length > 0) {
      chips.push({
        label: "60%+ match",
        active: goodMatchOnly,
        onToggle: () => (goodMatchOnly = !goodMatchOnly),
      });
    }

    // On before off, so what you have chosen reads left to right.
    return [...chips.filter((c) => c.active), ...chips.filter((c) => !c.active)];
  });

  const activeFilters = $derived(
    (discipline !== "All" ? 1 : 0) +
      sources.length +
      specialties.length +
      (modality !== "All" ? 1 : 0) +
      locations.length +
      (freshness !== "Any time" ? 1 : 0) +
      (goodMatchOnly ? 1 : 0),
  );

  function clearFilters() {
    discipline = "All";
    sources = [];
    specialties = [];
    modality = "All";
    locations = [];
    freshness = "Any time";
    goodMatchOnly = false;
  }

  const subtitle = $derived(
    feed.loading
      ? "Scanning sources…"
      : `${filtered.length.toLocaleString()} of ${items.length.toLocaleString()} open roles${
          activeFilters > 0 ? ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""}` : ""
        }`,
  );
</script>

{#snippet facet(label: string, options: readonly string[], current: string[], onToggle: (v: string) => void)}
  <div class="flex flex-col gap-2">
    <p class="text-[11px] font-bold tracking-wider text-subtle uppercase">{label}</p>
    <div class="flex flex-wrap gap-1.5">
      {#each options as option (option)}
        {@const active = current.includes(option)}
        <button
          onclick={() => onToggle(option)}
          aria-pressed={active}
          class="chip {active
            ? 'bg-brand text-brand-fg'
            : 'border border-line bg-surface text-muted hover:border-subtle'}"
        >
          {option}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<!--
  Below lg this is an ordinary scrolling page. At lg it becomes a fixed-height
  app pane so the two columns can own the scrolling and the detail pane's Apply
  button always sits on screen rather than hanging off the bottom.

  The height has to be stated outright. The shell around this is `min-h-screen`,
  which is a floor and not a size, so a chain of `flex-1`/`min-h-0` has nothing
  definite to shrink against and every child just grows to its content instead.
-->
<main
  class="mx-auto flex w-full max-w-7xl flex-1 flex-col px-4 pb-8 sm:px-6 lg:h-[calc(100dvh-58px)] lg:flex-none lg:overflow-hidden lg:px-8 lg:pb-5"
>
  <div class="shrink-0">
    <PageHeader title="Roles" {subtitle} />
  </div>

  <!-- One filter surface. Chips carry state; nothing hides in a sheet. -->
  <div class="shrink-0 pb-3">
    <div class="scrollbar-hide -mx-4 overflow-x-auto px-4 sm:mx-0 sm:overflow-visible sm:px-0">
      <div class="flex items-center gap-2 sm:flex-wrap">
        {#each quickChips as chip (chip.label)}
          <button
            onclick={chip.onToggle}
            aria-pressed={chip.active}
            class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3 text-[13px] transition-colors
                   {chip.active
              ? 'bg-brand font-semibold text-brand-fg'
              : 'border border-line bg-surface font-medium text-muted hover:border-subtle'}"
          >
            {chip.label}
            {#if chip.active}
              <X size={12} strokeWidth={2.5} class="shrink-0" />
            {/if}
          </button>
        {/each}

        <button
          onclick={() => (moreOpen = !moreOpen)}
          aria-expanded={moreOpen}
          aria-label="Filters"
          class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full border px-3 text-[13px] font-semibold transition-colors
                 {moreOpen
            ? 'border-brand bg-brand-soft text-brand-soft-fg'
            : 'border-line bg-surface text-muted hover:border-subtle'}"
        >
          <Filter size={13} strokeWidth={2.25} class="shrink-0" />
          Filters
          {#if activeFilters > 0}
            <span
              class="rounded-full px-1.5 text-[11px] font-bold tabular-nums
                     {moreOpen ? 'bg-brand-soft-fg/15' : 'bg-line text-subtle'}"
            >
              {activeFilters}
            </span>
          {/if}
          <ChevronDown
            size={13}
            strokeWidth={2.25}
            class="shrink-0 transition-transform {moreOpen ? 'rotate-180' : ''}"
          />
        </button>

        {#if activeFilters > 0}
          <button
            onclick={clearFilters}
            class="shrink-0 text-[13px] font-semibold text-brand hover:underline"
          >
            Clear all
          </button>
        {/if}

        <label class="ml-auto hidden shrink-0 items-center gap-1.5 text-[13px] font-medium text-muted sm:flex">
          <SlidersHorizontal size={14} class="shrink-0 text-subtle" />
          <span class="sr-only">Sort by</span>
          <select
            bind:value={sort}
            class="cursor-pointer appearance-none bg-transparent font-medium outline-none"
          >
            {#each HUB_SORTS as option (option)}
              <option value={option}>{option === "Relevance" ? "Best match" : option}</option>
            {/each}
          </select>
        </label>
      </div>
    </div>

    {#if moreOpen}
      <div class="animate-fade-in mt-3 flex flex-col gap-4 rounded-xl border border-line bg-surface p-4">
        {@render facet("Specialty", SPECIALTIES, specialties, (v) => (specialties = toggle(specialties, v)))}
        {@render facet("Work mode", MODALITIES.filter((m) => m !== "All"), modality === "All" ? [] : [modality], (v) => (modality = modality === v ? "All" : (v as Modality)))}
        {@render facet("Location", LOCATIONS, locations, (v) => (locations = toggle(locations, v)))}
        {@render facet("Posted within", FRESHNESS.map((f) => f.label), [freshness], (v) => (freshness = v as FreshnessLabel))}
        {@render facet("Discipline", HUB_DISCIPLINES, [discipline], (v) => (discipline = v))}
        {@render facet("Source", allSources, sources, (v) => (sources = toggle(sources, v)))}
      </div>
    {/if}
  </div>

  {#if feed.loading}
    <div class="card overflow-hidden p-2">
      <RowSkeleton count={8} />
    </div>
  {:else if filtered.length > 0}
    <!-- The two columns split the pane's height and scroll independently. -->
    <div
      class="lg:grid lg:min-h-0 lg:flex-1 lg:grid-cols-[minmax(0,25.25rem)_minmax(0,1fr)] lg:gap-5"
    >
      <div class="card overflow-hidden lg:h-full lg:overflow-y-auto">
        <div>
          {#each visible as item (item.url)}
            <ItemRow
              {item}
              variant={wide ? "dense" : "band"}
              selected={selected?.url === item.url}
              onOpen={(i) => (selected = i)}
            />
          {/each}
        </div>
        <InfiniteScroll {shown} total={filtered.length} noun="roles" onMore={() => (shown += PAGE)} />
      </div>

      {#if wide}
        <aside class="card flex h-full flex-col overflow-hidden bg-elevated">
          {#if selected}
            <ItemDetail item={selected} />
          {:else}
            <div class="flex flex-1 flex-col items-center justify-center px-8 text-center">
              <div
                class="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-line-soft text-subtle"
              >
                <Briefcase size={22} />
              </div>
              <p class="font-display text-lg font-bold">Pick a role</p>
              <p class="mt-1 max-w-xs text-sm text-muted">
                Choose anything on the left and the full posting opens here — no round trip through
                a dialog.
              </p>
            </div>
          {/if}
        </aside>
      {/if}
    </div>
  {:else if items.length === 0}
    <EmptyState
      icon={Briefcase}
      title="No roles cached yet"
      body="Refresh to pull the latest openings from all sources."
      actionLabel="Refresh now"
      busy={feed.refreshing}
      onAction={() => feed.refresh(true)}
    />
  {:else}
    <EmptyState
      icon={Search}
      title="No roles match"
      body="Nothing fits every chip at once. Try dropping the location or the work mode."
      actionLabel="Clear {activeFilters} filter{activeFilters === 1 ? '' : 's'}"
      onAction={clearFilters}
    />
  {/if}
</main>

<!-- Only below lg: at desktop widths the pane above is already showing this. -->
{#if !wide}
  <ItemModal item={selected} onClose={() => (selected = null)} />
{/if}
