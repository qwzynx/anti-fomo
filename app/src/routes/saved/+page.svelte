<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import ItemCard from "$lib/components/ItemCard.svelte";
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

<main class="mx-auto w-full max-w-6xl flex-1 px-4 py-8 sm:px-6 sm:py-10">
  <div class="mb-6">
    <h2 class="mb-1 text-3xl font-bold">Saved</h2>
    <p class="text-sm text-muted">
      {#if feed.saved.length === 0}
        Nothing saved yet.
      {:else}
        {filtered.length}
        {filtered.length === 1 ? "item" : "items"} · kept on this device even after they drop out of
        the feed
      {/if}
    </p>
  </div>

  {#if availableTypes.length > 2}
    <div class="scrollbar-hide mb-6 overflow-x-auto">
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
    <div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
      {#each filtered as item, idx (item.url)}
        <!-- Dismissing a saved item would be contradictory, so the card hides
             that action here. -->
        <ItemCard {item} index={idx} dismissable={false} onOpen={(i) => (selected = i)} />
      {/each}
    </div>
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
