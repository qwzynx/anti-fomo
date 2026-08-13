<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import FilterSheet from "$lib/components/FilterSheet.svelte";
  import InfiniteScroll from "$lib/components/InfiniteScroll.svelte";
  import ItemDetail from "$lib/components/ItemDetail.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Briefcase, ListFilter, Search } from "$lib/icons";
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
  import { splitTitle } from "$lib/item";

  // The job-board layout: a scrolling result list beside a detail pane that
  // stays put, which is what every board this app aggregates from converges on.
  // Comparing two postings there costs a glance rather than opening, reading,
  // closing and opening again. Below lg there is no room for two columns, so
  // the list goes full width and the detail opens as a modal instead.

  let selected = $state<ScrapedItem | null>(null);
  let filtersOpen = $state(false);

  let search = $state("");
  let discipline = $state("All");
  let sources = $state<string[]>([]);
  let specialties = $state<string[]>([]);
  let modality = $state<Modality>("All");
  let locations = $state<string[]>([]);
  let freshness = $state<FreshnessLabel>("Any time");
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
    const q = search.toLowerCase();
    const cutoff = freshnessCutoff(freshness);

    const result = items.filter((item) => {
      const haystack =
        `${item.title} ${item.content_text ?? ""} ${item.location ?? ""}`.toLowerCase();
      const locTags = item.location_tags ?? [];

      if (q && !haystack.includes(q)) return false;
      if (discipline !== "All" && item.discipline !== discipline) return false;
      if (sources.length > 0 && !sources.includes(item.source_platform)) return false;
      if (specialties.length > 0 && !specialties.some((s) => matchesSpecialty(haystack, s)))
        return false;
      if (modality !== "All" && !locTags.includes(modality)) return false;
      if (locations.length > 0 && !locations.some((l) => locTags.includes(l))) return false;
      if (cutoff !== -Infinity && new Date(item.timestamp).getTime() < cutoff) return false;
      return true;
    });

    switch (sort) {
      case "Newest first":
        return [...result].sort((a, b) => b.timestamp.localeCompare(a.timestamp));
      case "Company name":
        return [...result].sort((a, b) =>
          splitTitle(a).primary.localeCompare(splitTitle(b).primary),
        );
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

  const activeFilters = $derived(
    (discipline !== "All" ? 1 : 0) +
      sources.length +
      specialties.length +
      (modality !== "All" ? 1 : 0) +
      locations.length +
      (freshness !== "Any time" ? 1 : 0),
  );

  function clearFilters() {
    discipline = "All";
    sources = [];
    specialties = [];
    modality = "All";
    locations = [];
    freshness = "Any time";
  }

  function chipClass(active: boolean) {
    return `chip ${
      active
        ? "bg-brand text-brand-fg"
        : "border border-line bg-surface text-muted hover:border-brand"
    }`;
  }
</script>

{#snippet chipRow(label: string, options: string[], current: string[], onToggle: (v: string) => void)}
  <div class="flex flex-col gap-2">
    <p class="text-xs font-bold tracking-wide text-subtle uppercase">{label}</p>
    <div class="flex flex-wrap gap-1.5">
      {#each options as option (option)}
        <button onclick={() => onToggle(option)} class={chipClass(current.includes(option))}>
          {option}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

{#snippet selects()}
  <select bind:value={discipline} class="control control-focus" aria-label="Discipline">
    {#each HUB_DISCIPLINES as option (option)}
      <option value={option}>{option === "All" ? "All disciplines" : option}</option>
    {/each}
  </select>
  <select bind:value={freshness} class="control control-focus" aria-label="Posted within">
    {#each FRESHNESS as option (option.label)}
      <option value={option.label}>{option.label}</option>
    {/each}
  </select>
  <select bind:value={sort} class="control control-focus" aria-label="Sort by">
    {#each HUB_SORTS as option (option)}
      <option value={option}>{option}</option>
    {/each}
  </select>
{/snippet}

{#snippet facets()}
  {@render chipRow("Specialty", SPECIALTIES, specialties, (v) => (specialties = toggle(specialties, v)))}

  <div class="flex flex-col gap-2">
    <p class="text-xs font-bold tracking-wide text-subtle uppercase">Work mode</p>
    <div class="flex flex-wrap gap-1.5">
      {#each MODALITIES as option (option)}
        <button onclick={() => (modality = option)} class={chipClass(modality === option)}>
          {option}
        </button>
      {/each}
    </div>
  </div>

  {@render chipRow("Location", LOCATIONS, locations, (v) => (locations = toggle(locations, v)))}
  {@render chipRow("Source", allSources, sources, (v) => (sources = toggle(sources, v)))}
{/snippet}

{#snippet resultList()}
  <div class="divide-y divide-line-soft">
    {#each visible as item (item.url)}
      <ItemRow
        {item}
        selected={selected?.url === item.url}
        onOpen={(i) => (selected = i)}
      />
    {/each}
  </div>
  <InfiniteScroll {shown} total={filtered.length} noun="roles" onMore={() => (shown += PAGE)} />
{/snippet}

<!--
  Below lg this is an ordinary scrolling page. At lg it becomes a fixed-height
  app pane so the two columns can own the scrolling and the detail pane's Apply
  button always sits on screen rather than hanging off the bottom.

  The height has to be stated outright. The shell around this is `min-h-screen`,
  which is a floor and not a size, so a chain of `flex-1`/`min-h-0` has nothing
  definite to shrink against and every child just grows to its content instead.
  `flex-none` goes with it because `flex-1` would otherwise resolve the height
  from `flex-basis: 0%` and ignore `h-dvh` entirely.
-->
<main
  class="mx-auto flex w-full max-w-7xl flex-1 flex-col px-4 pb-8 sm:px-6 lg:h-dvh lg:flex-none lg:overflow-hidden lg:px-8 lg:pb-5"
>
  <div class="shrink-0 pt-6 pb-4 sm:pt-8">
    <h1 class="text-2xl font-bold sm:text-3xl">Jobs &amp; internships</h1>
    <p class="mt-1 text-sm text-muted">
      {#if feed.loading}
        Scanning sources…
      {:else}
        {filtered.length.toLocaleString()} of {items.length.toLocaleString()} open roles from {allSources.length}
        sources{activeFilters > 0
          ? ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""} active`
          : ""}
      {/if}
    </p>
  </div>

  <!-- Toolbar. Sticky on phones, where it scrolls with a single long list;
       static on desktop, where the columns below do their own scrolling and a
       sticky bar would only collide with them. -->
  <div
    class="glass sticky top-14 z-30 -mx-4 shrink-0 border-y border-line px-4 py-3 sm:-mx-6 sm:px-6 md:top-0 lg:static lg:mx-0 lg:rounded-2xl lg:border lg:border-line lg:px-4"
  >
    <!-- Wraps rather than squeezing: beside the sidebar there is not room for
         the search box, three selects and the filter button on one line, and a
         search field crushed to 150px is worse than a second row. -->
    <div class="flex flex-wrap items-center gap-2">
      <div class="relative min-w-56 flex-1">
        <Search
          size={16}
          class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-subtle"
        />
        <input
          type="search"
          placeholder="Search roles, companies, locations…"
          aria-label="Search roles"
          bind:value={search}
          class="control control-focus w-full py-2.5 pl-9"
        />
      </div>

      <div class="hidden items-center gap-2 lg:flex">
        {@render selects()}
        <button
          onclick={() => (filtersOpen = !filtersOpen)}
          aria-expanded={filtersOpen}
          class="control control-focus flex shrink-0 items-center gap-2 font-semibold"
        >
          <ListFilter size={15} />
          Filters
          {#if activeFilters > 0}
            <span class="rounded-full bg-brand px-1.5 text-[11px] font-bold text-brand-fg">
              {activeFilters}
            </span>
          {/if}
        </button>
      </div>

      <div class="lg:hidden">
        <FilterSheet activeCount={activeFilters} onReset={clearFilters}>
          <div class="flex flex-col gap-3">
            {@render selects()}
          </div>
          {@render facets()}
        </FilterSheet>
      </div>
    </div>

    {#if filtersOpen}
      <div class="animate-fade-in mt-4 hidden flex-col gap-4 border-t border-line-soft pt-4 lg:flex">
        {@render facets()}
        {#if activeFilters > 0}
          <button
            onclick={clearFilters}
            class="self-start text-xs font-semibold text-brand hover:underline"
          >
            Clear {activeFilters} filter{activeFilters > 1 ? "s" : ""}
          </button>
        {/if}
      </div>
    {/if}
  </div>

  {#if feed.loading}
    <div class="card mt-5 divide-y divide-line-soft overflow-hidden">
      {#each { length: 8 } as _, i (i)}
        <div class="flex animate-pulse items-start gap-3 px-4 py-3.5" style="animation-delay: {i * 60}ms">
          <div class="h-9 w-9 shrink-0 rounded-lg bg-line"></div>
          <div class="flex-1">
            <div class="mb-2 h-4 w-2/5 rounded bg-line"></div>
            <div class="h-3 w-1/4 rounded bg-line-soft"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if filtered.length > 0}
    <!-- The two columns split the pane's height and scroll independently. -->
    <div
      class="mt-5 lg:grid lg:min-h-0 lg:flex-1 lg:grid-cols-[minmax(0,26rem)_minmax(0,1fr)] lg:gap-5 xl:grid-cols-[minmax(0,30rem)_minmax(0,1fr)]"
    >
      <div class="card overflow-hidden lg:h-full lg:overflow-y-auto">
        {@render resultList()}
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
    <div class="mt-5">
      <EmptyState
        icon={Briefcase}
        title="No roles cached yet"
        body="Refresh to pull the latest openings from all sources."
        actionLabel="Refresh now"
        busy={feed.refreshing}
        onAction={() => feed.refresh(true)}
      />
    </div>
  {:else}
    <div class="mt-5">
      <EmptyState
        icon={Search}
        title="No roles match"
        body="Try loosening the location, specialty or freshness filters."
        actionLabel="Clear filters"
        onAction={() => {
          search = "";
          clearFilters();
        }}
      />
    </div>
  {/if}
</main>

<!-- Only below lg: at desktop widths the pane above is already showing this. -->
{#if !wide}
  <ItemModal item={selected} onClose={() => (selected = null)} />
{/if}
