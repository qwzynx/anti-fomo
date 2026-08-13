<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import DensityToggle from "$lib/components/DensityToggle.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import ItemList from "$lib/components/ItemList.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark } from "$lib/icons";
  import { TYPE_MAP, TYPE_OPTIONS, type TypeOption } from "$lib/filters";

  let selected = $state<ScrapedItem | null>(null);
  let itemType = $state<TypeOption>("All");

  const filtered = $derived(
    itemType === "All"
      ? feed.saved
      : feed.saved.filter((i) => TYPE_MAP[itemType].includes(i.item_type)),
  );

  /** Only offer a type filter for types actually present in the list. */
  const availableTypes = $derived(
    TYPE_OPTIONS.filter(
      (t) => t === "All" || feed.saved.some((i) => TYPE_MAP[t].includes(i.item_type)),
    ),
  );
</script>

<main class="mx-auto w-full max-w-6xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <div class="flex flex-wrap items-end justify-between gap-3 pt-6 pb-5 sm:pt-8">
    <div>
      <h1 class="text-2xl font-bold sm:text-3xl">Saved</h1>
      <p class="mt-1 text-sm text-muted">
        {#if feed.saved.length === 0}
          Nothing saved yet.
        {:else}
          {filtered.length}
          {filtered.length === 1 ? "item" : "items"} · kept on this device even after they drop out
          of the feed
        {/if}
      </p>
    </div>
    {#if feed.saved.length > 0}
      <DensityToggle />
    {/if}
  </div>

  {#if availableTypes.length > 2}
    <div class="scrollbar-hide mb-5 overflow-x-auto">
      <div class="flex gap-2">
        {#each availableTypes as option (option)}
          <button
            onclick={() => (itemType = option)}
            aria-pressed={itemType === option}
            class="chip {itemType === option
              ? 'bg-brand text-brand-fg'
              : 'border border-line bg-surface text-muted hover:border-brand'}"
          >
            {option === "All" ? "All" : option}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if filtered.length > 0}
    <!-- Dismissing a saved item would be contradictory, so the cards and rows
         hide that action here. -->
    <ItemList
      items={filtered}
      dismissable={false}
      noun="saved items"
      onOpen={(i) => (selected = i)}
    />
  {:else if feed.saved.length > 0}
    <EmptyState
      icon={Bookmark}
      title="Nothing of that type"
      body="You have saved items, just none in this category."
      actionLabel="Show all"
      onAction={() => (itemType = "All")}
    />
  {:else}
    <EmptyState
      icon={Bookmark}
      title="No saved items"
      body="Tap the bookmark on any card to keep it here. Saved items stick around even after the listing ages out of the feed."
    />
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
