<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import FilterSheet from "$lib/components/FilterSheet.svelte";
  import ItemCard from "$lib/components/ItemCard.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import { feed } from "$lib/feed.svelte";
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

  let selected = $state<ScrapedItem | null>(null);

  let search = $state("");
  let discipline = $state("All");
  let sources = $state<string[]>([]);
  let specialties = $state<string[]>([]);
  let modality = $state<Modality>("All");
  let locations = $state<string[]>([]);
  let freshness = $state<FreshnessLabel>("Any time");
  let sort = $state<HubSort>("Relevance");

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

  const selectClass =
    "rounded-xl border border-zinc-200 bg-white px-3 py-2 text-sm font-medium outline-none focus:ring-2 focus:ring-indigo-500 dark:border-zinc-800 dark:bg-zinc-900";

  function chipClass(active: boolean) {
    return `rounded-full px-3 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors ${
      active
        ? "bg-indigo-600 text-white"
        : "border border-zinc-200 bg-white text-zinc-600 hover:border-indigo-400 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400"
    }`;
  }
</script>

{#snippet chipRow(label: string, options: string[], current: string[], onToggle: (v: string) => void)}
  <div class="flex flex-col gap-2">
    <p class="text-xs font-bold tracking-wide text-zinc-400 uppercase">{label}</p>
    <div class="scrollbar-hide flex flex-wrap gap-1.5">
      {#each options as option (option)}
        <button onclick={() => onToggle(option)} class={chipClass(current.includes(option))}>
          {option}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

{#snippet controls()}
  <div class="flex flex-col gap-3 md:flex-row">
    <input
      type="text"
      placeholder="Search roles, companies, locations…"
      bind:value={search}
      class="flex-1 rounded-xl border border-zinc-200 bg-white px-4 py-2 outline-none focus:ring-2 focus:ring-indigo-500 dark:border-zinc-800 dark:bg-zinc-900"
    />
    <select bind:value={discipline} class={selectClass}>
      {#each HUB_DISCIPLINES as option (option)}
        <option value={option}>{option === "All" ? "All disciplines" : option}</option>
      {/each}
    </select>
    <select bind:value={freshness} class={selectClass}>
      {#each FRESHNESS as option (option.label)}
        <option value={option.label}>⏱️ {option.label}</option>
      {/each}
    </select>
    <select bind:value={sort} class={selectClass}>
      {#each HUB_SORTS as option (option)}
        <option value={option}>📊 {option}</option>
      {/each}
    </select>
  </div>

  {@render chipRow("Specialty", SPECIALTIES, specialties, (v) => (specialties = toggle(specialties, v)))}

  <div class="flex flex-col gap-2">
    <p class="text-xs font-bold tracking-wide text-zinc-400 uppercase">Work mode</p>
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

<main class="mx-auto w-full max-w-6xl flex-1 px-4 py-8 sm:px-6 sm:py-10">
  <div class="mb-6">
    <h2 class="mb-1 text-3xl font-bold">💼 Internship Hub</h2>
    <p class="text-sm text-zinc-500 dark:text-zinc-400">
      {feed.loading
        ? "Scanning sources…"
        : `${filtered.length} of ${items.length} open roles from ${allSources.length} sources`}{activeFilters >
      0
        ? ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""} active`
        : ""}
    </p>
  </div>

  <!-- Desktop: the full panel. Phones: the same controls in a sheet. -->
  <div
    class="mb-8 hidden flex-col gap-4 rounded-2xl border border-zinc-200 bg-white p-5 sm:flex dark:border-zinc-800 dark:bg-zinc-900"
  >
    {@render controls()}
    {#if activeFilters > 0}
      <button
        onclick={clearFilters}
        class="self-start text-xs font-semibold text-indigo-600 hover:underline dark:text-indigo-400"
      >
        Clear filters
      </button>
    {/if}
  </div>

  <div class="mb-6 flex flex-col gap-3 sm:hidden">
    <input
      type="text"
      placeholder="Search roles, companies…"
      bind:value={search}
      class="w-full rounded-xl border border-zinc-200 bg-white px-4 py-2.5 outline-none focus:ring-2 focus:ring-indigo-500 dark:border-zinc-800 dark:bg-zinc-900"
    />
    <FilterSheet activeCount={activeFilters} onReset={clearFilters}>
      {@render controls()}
    </FilterSheet>
  </div>

  {#if feed.loading}
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each { length: 9 } as _, i (i)}
        <div class="h-52 w-full animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800"></div>
      {/each}
    </div>
  {:else if filtered.length > 0}
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each filtered as item, idx (item.url)}
        <ItemCard {item} index={idx} onOpen={(i) => (selected = i)} />
      {/each}
    </div>
  {:else}
    <div
      class="rounded-3xl border border-dashed border-zinc-300 bg-white py-20 text-center dark:border-zinc-700 dark:bg-zinc-900"
    >
      <p class="text-zinc-500 dark:text-zinc-400">No roles match these filters.</p>
      {#if activeFilters > 0 || search}
        <button
          onclick={() => {
            search = "";
            clearFilters();
          }}
          class="mt-4 text-sm font-semibold text-indigo-600 hover:underline"
        >
          Clear filters
        </button>
      {/if}
    </div>
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
