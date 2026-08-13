<script lang="ts">
  import { timeAgo, type ScrapedItem } from "$lib/api";
  import CardSkeleton from "$lib/components/CardSkeleton.svelte";
  import DensityToggle from "$lib/components/DensityToggle.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import FilterSheet from "$lib/components/FilterSheet.svelte";
  import ItemList from "$lib/components/ItemList.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Calendar, Flame, Inbox, Search, Sparkles, TrendingUp } from "$lib/icons";
  import {
    DISCIPLINES,
    FRESHNESS,
    NEWS_SOURCES,
    SORTS,
    TYPE_MAP,
    TYPE_OPTIONS,
    freshnessCutoff,
    type FreshnessLabel,
    type Sort,
    type TypeOption,
  } from "$lib/filters";

  let searchTerm = $state("");
  let selectedDiscipline = $state("All");
  let selected = $state<ScrapedItem | null>(null);
  let itemType = $state<TypeOption>("All");
  let sourceFilter = $state("All");
  let freshness = $state<FreshnessLabel>("Any time");
  let sort = $state<Sort>("Relevance");

  const topMatch = $derived(
    feed.items.find((i) => (i.item_type === "Internship" || i.item_type === "Job") && !i.seen) ??
      null,
  );

  const trending = $derived(
    feed.items
      .filter((i) => i.item_type === "Article" && NEWS_SOURCES.includes(i.source_platform))
      .slice(0, 3),
  );

  /** Soonest upcoming events — the category the widgets never covered before. */
  const upcoming = $derived(
    feed.items
      .filter((i) => i.item_type === "Event" && new Date(i.timestamp).getTime() >= Date.now())
      .sort((a, b) => a.timestamp.localeCompare(b.timestamp))
      .slice(0, 3),
  );

  const allSources = $derived([...new Set(feed.items.map((i) => i.source_platform))].sort());

  const filteredItems = $derived.by(() => {
    const cutoff = freshnessCutoff(freshness);
    const needle = searchTerm.toLowerCase();

    const result = feed.items.filter((item) => {
      const matchesSearch =
        item.title.toLowerCase().includes(needle) ||
        (item.content_text && item.content_text.toLowerCase().includes(needle));
      if (!matchesSearch) return false;
      if (selectedDiscipline !== "All" && item.discipline !== selectedDiscipline) return false;
      if (itemType !== "All" && !TYPE_MAP[itemType].includes(item.item_type)) return false;
      if (sourceFilter !== "All" && item.source_platform !== sourceFilter) return false;
      if (cutoff !== -Infinity && new Date(item.timestamp).getTime() < cutoff) return false;
      return true;
    });

    // The Rust side already returns relevance order.
    return sort === "Newest first"
      ? [...result].sort((a, b) => b.timestamp.localeCompare(a.timestamp))
      : result;
  });

  const activeFilters = $derived(
    (itemType !== "All" ? 1 : 0) +
      (sourceFilter !== "All" ? 1 : 0) +
      (freshness !== "Any time" ? 1 : 0) +
      (sort !== "Relevance" ? 1 : 0),
  );

  function resetFilters() {
    itemType = "All";
    sourceFilter = "All";
    freshness = "Any time";
    sort = "Relevance";
  }

  function clearEverything() {
    searchTerm = "";
    selectedDiscipline = "All";
    resetFilters();
  }
</script>

{#snippet widget(
  label: string,
  Icon: typeof Flame,
  accent: string,
  entries: ScrapedItem[],
  emptyText: string,
  delay: number,
)}
  <div class="animate-fade-up card p-5" style="animation-delay: {delay}ms">
    <p class="mb-2.5 flex items-center gap-1.5 text-xs font-bold tracking-wider uppercase {accent}">
      <Icon size={13} strokeWidth={2.5} />
      {label}
    </p>
    {#if entries.length > 0}
      <ul class="flex flex-col gap-2">
        {#each entries as entry (entry.url)}
          <li class="text-sm leading-snug">
            <button
              onclick={() => (selected = entry)}
              class="line-clamp-2 text-left font-semibold hover:text-brand"
            >
              {entry.title}
            </button>
            <span class="text-xs text-subtle">
              {entry.source_platform} · {timeAgo(entry.timestamp)}
            </span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="text-sm text-muted">{feed.loading ? "Scanning sources…" : emptyText}</p>
    {/if}
  </div>
{/snippet}

{#snippet filterControls()}
  <select bind:value={itemType} class="control control-focus" aria-label="Item type">
    {#each TYPE_OPTIONS as option (option)}
      <option value={option}>{option === "All" ? "All types" : option}</option>
    {/each}
  </select>
  <select bind:value={sourceFilter} class="control control-focus" aria-label="Source">
    <option value="All">All sources</option>
    {#each allSources as source (source)}
      <option value={source}>{source}</option>
    {/each}
  </select>
  <select bind:value={freshness} class="control control-focus" aria-label="Posted within">
    {#each FRESHNESS as option (option.label)}
      <option value={option.label}>{option.label}</option>
    {/each}
  </select>
  <select bind:value={sort} class="control control-focus" aria-label="Sort by">
    {#each SORTS as option (option)}
      <option value={option}>{option}</option>
    {/each}
  </select>
{/snippet}

<main class="mx-auto w-full max-w-6xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <div class="pt-6 pb-5 sm:pt-8">
    <h1 class="text-2xl font-bold sm:text-3xl">Your feed</h1>
    <p class="mt-1 text-sm text-muted">
      {#if feed.loading}
        Scanning {feed.status?.sources.length ?? 0} sources…
      {:else}
        {filteredItems.length.toLocaleString()} results for {feed.major} students{selectedDiscipline !==
        "All"
          ? ` · filtered by ${selectedDiscipline}`
          : ""}
      {/if}
    </p>
  </div>

  <!-- Curated widgets: one per category, so news never crowds out the other two -->
  <section class="mb-6 grid grid-cols-1 gap-4 lg:grid-cols-3">
    {@render widget(
      "Top match today",
      Flame,
      "text-star",
      topMatch ? [topMatch] : [],
      "No strong match yet — check back soon.",
      0,
    )}
    {@render widget("Trending in tech", TrendingUp, "text-brand", trending, "No stories right now.", 60)}
    {@render widget("Happening soon", Calendar, "text-event", upcoming, "No upcoming events.", 120)}
  </section>

  {#if feed.interests.length === 0 && !feed.loading}
    <a
      href="/settings"
      class="animate-fade-in mb-5 flex items-center gap-2.5 rounded-xl border border-brand/30 bg-brand-soft px-4 py-3 text-sm text-brand-soft-fg transition-colors hover:border-brand"
    >
      <Sparkles size={16} class="shrink-0" />
      <span>
        <span class="font-semibold">Pick your interests</span> to rank the feed around what you
        actually care about.
      </span>
    </a>
  {/if}

  <!--
    Sticky toolbar. It carries search, the filters and the density switch, and
    stays reachable however far down the list the reader has scrolled — which
    matters a great deal more now that the list runs to hundreds of results
    rather than sixty. The negative margins let it span the full column width
    while its contents stay aligned with the rest of the page.
  -->
  <div
    class="glass sticky top-14 z-30 -mx-4 mb-4 border-y border-line px-4 py-3 sm:-mx-6 sm:px-6 md:top-0 lg:-mx-8 lg:px-8"
  >
    <!-- Wraps rather than squeezing: four selects, a search box and the density
         switch do not fit on one line beside the sidebar at 1024px, and a
         search field crushed to 150px is worse than a second row. -->
    <div class="flex flex-wrap items-center gap-2">
      <div class="relative min-w-56 flex-1">
        <Search
          size={16}
          class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-subtle"
        />
        <input
          type="search"
          placeholder="Search your feed…"
          aria-label="Search your feed"
          bind:value={searchTerm}
          class="control control-focus w-full py-2.5 pl-9"
        />
      </div>

      <!-- Filters are inline on desktop and behind a sheet on phones. -->
      <div class="hidden flex-wrap items-center gap-2 lg:flex">
        {@render filterControls()}
      </div>
      <div class="lg:hidden">
        <FilterSheet activeCount={activeFilters} onReset={resetFilters}>
          <div class="flex flex-col gap-3">
            {@render filterControls()}
          </div>
        </FilterSheet>
      </div>

      <DensityToggle />
    </div>

    {#if activeFilters > 0}
      <button
        onclick={resetFilters}
        class="mt-2 text-xs font-semibold text-brand hover:underline lg:mt-1.5"
      >
        Reset {activeFilters} filter{activeFilters > 1 ? "s" : ""}
      </button>
    {/if}
  </div>

  <div class="scrollbar-hide mb-5 overflow-x-auto">
    <div class="flex gap-2">
      {#each DISCIPLINES as discipline (discipline)}
        <button
          onclick={() => (selectedDiscipline = discipline)}
          aria-pressed={selectedDiscipline === discipline}
          class="chip {selectedDiscipline === discipline
            ? 'bg-brand text-brand-fg'
            : 'border border-line bg-surface text-muted hover:border-brand'}"
        >
          {discipline}
        </button>
      {/each}
    </div>
  </div>

  <!-- Feed -->
  {#if feed.loading}
    <CardSkeleton count={6} />
  {:else if filteredItems.length > 0}
    <ItemList items={filteredItems} onOpen={(i) => (selected = i)} />
  {:else if feed.items.length === 0}
    <EmptyState
      icon={Inbox}
      title="Nothing cached yet"
      body="Pull the latest listings, stories and events from your sources."
      actionLabel="Refresh now"
      busy={feed.refreshing}
      onAction={() => feed.refresh(true)}
    />
  {:else}
    <EmptyState
      icon={Search}
      title="No matches"
      body="Nothing in your feed matches this search and filter combination."
      actionLabel="Clear all filters"
      onAction={clearEverything}
    />
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
