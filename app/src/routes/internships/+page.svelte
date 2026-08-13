<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import CardSkeleton from "$lib/components/CardSkeleton.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import FilterSheet from "$lib/components/FilterSheet.svelte";
  import ItemCard from "$lib/components/ItemCard.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Briefcase, Search } from "$lib/icons";
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

  const selectClass = "control control-focus";

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
      class="control control-focus flex-1 py-2.5"
    />
    <select bind:value={discipline} class={selectClass}>
      {#each HUB_DISCIPLINES as option (option)}
        <option value={option}>{option === "All" ? "All disciplines" : option}</option>
      {/each}
    </select>
    <select bind:value={freshness} class={selectClass}>
      {#each FRESHNESS as option (option.label)}
        <option value={option.label}>{option.label}</option>
      {/each}
    </select>
    <select bind:value={sort} class={selectClass}>
      {#each HUB_SORTS as option (option)}
        <option value={option}>{option}</option>
      {/each}
    </select>
  </div>

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

<main class="mx-auto w-full max-w-6xl flex-1 px-4 py-8 sm:px-6 sm:py-10">
  <div class="mb-6">
    <h2 class="mb-1 text-3xl font-bold">Jobs &amp; internships</h2>
    <p class="text-sm text-muted">
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
    class="card mb-8 hidden flex-col gap-4 p-5 sm:flex"
  >
    {@render controls()}
    {#if activeFilters > 0}
      <button
        onclick={clearFilters}
        class="self-start text-xs font-semibold text-brand hover:underline"
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
      class="control control-focus w-full py-2.5"
    />
    <FilterSheet activeCount={activeFilters} onReset={clearFilters}>
      {@render controls()}
    </FilterSheet>
  </div>

  {#if feed.loading}
    <CardSkeleton count={9} />
  {:else if filtered.length > 0}
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each filtered as item, idx (item.url)}
        <ItemCard {item} index={idx} onOpen={(i) => (selected = i)} />
      {/each}
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
      body="Try loosening the location, specialty or freshness filters."
      actionLabel="Clear filters"
      onAction={() => {
        search = "";
        clearFilters();
      }}
    />
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
