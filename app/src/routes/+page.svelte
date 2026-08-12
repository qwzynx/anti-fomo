<script lang="ts">
  import { timeAgo, type ScrapedItem } from "$lib/api";
  import FilterSheet from "$lib/components/FilterSheet.svelte";
  import ItemCard from "$lib/components/ItemCard.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import { feed } from "$lib/feed.svelte";
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
    feed.items.find(
      (i) =>
        (i.item_type === "Internship" || i.item_type === "Job") && (i.relevance_score ?? 0) >= 10,
    ) ?? null,
  );

  const trending = $derived(
    feed.items
      .filter((i) => i.item_type === "Article" && NEWS_SOURCES.includes(i.source_platform))
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

  const filtersActive = $derived(
    itemType !== "All" ||
      sourceFilter !== "All" ||
      freshness !== "Any time" ||
      sort !== "Relevance",
  );

  function resetFilters() {
    itemType = "All";
    sourceFilter = "All";
    freshness = "Any time";
    sort = "Relevance";
  }

  const selectClass =
    "rounded-xl border border-zinc-200 bg-white px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500 dark:border-zinc-800 dark:bg-zinc-900";
</script>

<main class="mx-auto w-full max-w-6xl flex-1 px-4 py-8 sm:px-6 sm:py-10">
  <!-- Curated widgets -->
  <section class="mb-8 grid grid-cols-1 gap-4 md:grid-cols-2">
    <div
      class="animate-fade-up rounded-2xl border border-amber-200/60 bg-gradient-to-br from-amber-50 to-white p-5 dark:border-amber-900/40 dark:from-amber-950/30 dark:to-zinc-900"
    >
      <p class="mb-2 text-xs font-bold tracking-wider text-amber-600 uppercase dark:text-amber-400">
        🔥 Top Match Today
      </p>
      {#if topMatch}
        <button onclick={() => (selected = topMatch)} class="group text-left">
          <p
            class="leading-snug font-bold group-hover:text-indigo-600 dark:group-hover:text-indigo-400"
          >
            {topMatch.title}
          </p>
          <p class="mt-1 text-xs text-zinc-500">
            {topMatch.source_platform} · {topMatch.discipline}
          </p>
        </button>
      {:else}
        <p class="text-sm text-zinc-500">
          {feed.loading ? "Scanning sources…" : "No strong match yet — check back soon."}
        </p>
      {/if}
    </div>

    <div
      class="animate-fade-up rounded-2xl border border-indigo-200/60 bg-gradient-to-br from-indigo-50 to-white p-5 dark:border-indigo-900/40 dark:from-indigo-950/30 dark:to-zinc-900"
      style="animation-delay: 60ms"
    >
      <p
        class="mb-2 text-xs font-bold tracking-wider text-indigo-600 uppercase dark:text-indigo-400"
      >
        📰 Trending in Tech
      </p>
      {#if trending.length > 0}
        <ul class="flex flex-col gap-1.5">
          {#each trending as story (story.url)}
            <li class="text-sm leading-snug">
              <button
                onclick={() => (selected = story)}
                class="line-clamp-1 text-left font-semibold hover:text-indigo-600 dark:hover:text-indigo-400"
              >
                {story.title}
              </button>
              <span class="text-xs text-zinc-500">
                {story.source_platform} · {timeAgo(story.timestamp)}
              </span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="text-sm text-zinc-500">
          {feed.loading ? "Loading stories…" : "No stories right now."}
        </p>
      {/if}
    </div>
  </section>

  <!-- Controls -->
  <div class="mb-5 flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
    <div>
      <h2 class="mb-1 text-3xl font-bold">Your Feed</h2>
      <p class="text-sm text-zinc-500 dark:text-zinc-400">
        {filteredItems.length} results for {feed.major} students{selectedDiscipline !== "All"
          ? ` · filtered by ${selectedDiscipline}`
          : ""}
      </p>
    </div>
    <input
      type="text"
      placeholder="Search your feed…"
      bind:value={searchTerm}
      class="w-full rounded-xl border border-zinc-200 bg-white px-4 py-2 transition-all outline-none focus:ring-2 focus:ring-indigo-500 md:w-64 dark:border-zinc-800 dark:bg-zinc-900"
    />
  </div>

  <!-- Filters: inline on desktop, a sheet on phones -->
  <div class="mb-4 hidden flex-wrap items-center gap-2 sm:flex">
    <select bind:value={itemType} class={selectClass}>
      {#each TYPE_OPTIONS as option (option)}
        <option value={option}>📦 {option === "All" ? "All types" : option}</option>
      {/each}
    </select>
    <select bind:value={sourceFilter} class={selectClass}>
      <option value="All">🌐 All sources</option>
      {#each allSources as source (source)}
        <option value={source}>{source}</option>
      {/each}
    </select>
    <select bind:value={freshness} class={selectClass}>
      {#each FRESHNESS as option (option.label)}
        <option value={option.label}>⏱️ {option.label}</option>
      {/each}
    </select>
    <select bind:value={sort} class={selectClass}>
      {#each SORTS as option (option)}
        <option value={option}>📊 {option}</option>
      {/each}
    </select>
    {#if filtersActive}
      <button
        onclick={resetFilters}
        class="text-xs font-semibold text-indigo-600 hover:underline dark:text-indigo-400"
      >
        Reset
      </button>
    {/if}
  </div>

  <div class="mb-4 sm:hidden">
    <FilterSheet activeCount={filtersActive ? 1 : 0} onReset={resetFilters}>
      <label class="flex flex-col gap-1 text-sm font-semibold">
        Type
        <select bind:value={itemType} class={selectClass}>
          {#each TYPE_OPTIONS as option (option)}
            <option value={option}>{option === "All" ? "All types" : option}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col gap-1 text-sm font-semibold">
        Source
        <select bind:value={sourceFilter} class={selectClass}>
          <option value="All">All sources</option>
          {#each allSources as source (source)}
            <option value={source}>{source}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col gap-1 text-sm font-semibold">
        Posted
        <select bind:value={freshness} class={selectClass}>
          {#each FRESHNESS as option (option.label)}
            <option value={option.label}>{option.label}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col gap-1 text-sm font-semibold">
        Sort
        <select bind:value={sort} class={selectClass}>
          {#each SORTS as option (option)}
            <option value={option}>{option}</option>
          {/each}
        </select>
      </label>
    </FilterSheet>
  </div>

  <div class="scrollbar-hide mb-6 overflow-x-auto">
    <div class="flex gap-2">
      {#each DISCIPLINES as discipline (discipline)}
        <button
          onclick={() => (selectedDiscipline = discipline)}
          class="rounded-full px-3.5 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors {selectedDiscipline ===
          discipline
            ? 'bg-indigo-600 text-white'
            : 'border border-zinc-200 bg-white text-zinc-600 hover:border-indigo-400 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400'}"
        >
          {discipline}
        </button>
      {/each}
    </div>
  </div>

  <!-- Feed -->
  {#if feed.loading}
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each { length: 6 } as _, i (i)}
        <div class="h-52 w-full animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800"></div>
      {/each}
    </div>
  {:else if filteredItems.length > 0}
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each filteredItems as item, idx (item.url)}
        <ItemCard {item} index={idx} onOpen={(i) => (selected = i)} />
      {/each}
    </div>
  {:else}
    <div
      class="rounded-3xl border border-dashed border-zinc-300 bg-white py-20 text-center dark:border-zinc-700 dark:bg-zinc-900"
    >
      <p class="text-zinc-500 dark:text-zinc-400">
        {feed.items.length === 0
          ? "Nothing cached yet — pull the latest from your sources."
          : "No opportunities match your search or filters."}
      </p>
      {#if feed.items.length === 0}
        <button
          onclick={() => feed.refresh(true)}
          disabled={feed.refreshing}
          class="mt-4 rounded-xl bg-indigo-600 px-5 py-2.5 text-sm font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
        >
          {feed.refreshing ? "Refreshing…" : "Refresh now"}
        </button>
      {:else}
        <button
          onclick={() => {
            searchTerm = "";
            selectedDiscipline = "All";
            resetFilters();
          }}
          class="mt-4 text-sm font-semibold text-indigo-600 hover:underline"
        >
          Clear all filters
        </button>
      {/if}
    </div>
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
