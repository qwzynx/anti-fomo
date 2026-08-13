<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { density } from "$lib/density.svelte";
  import InfiniteScroll from "./InfiniteScroll.svelte";
  import ItemCard from "./ItemCard.svelte";
  import ItemRow from "./ItemRow.svelte";

  // One results list, three shapes, shared by the feed and the saved page. The
  // density choice lives in its own store rather than being passed down,
  // because the toolbar that sets it and the list that reads it are never in
  // the same component.
  let {
    items,
    onOpen,
    dismissable = true,
    noun = "results",
  }: {
    items: ScrapedItem[];
    onOpen: (item: ScrapedItem) => void;
    dismissable?: boolean;
    noun?: string;
  } = $props();

  const PAGE = 30;

  let shown = $state(PAGE);

  // Length plus the head item identifies "a different list" well enough to know
  // when to send the reader back to the top of it: changing a filter moves one
  // or the other, while a background refresh that lands on the same filter
  // usually moves neither, and paging survives it.
  const signature = $derived(`${items.length}|${items[0]?.url ?? ""}`);

  $effect(() => {
    void signature;
    shown = PAGE;
  });

  const visible = $derived(items.slice(0, shown));
</script>

{#if density.value === "cards"}
  <div class="grid grid-cols-1 gap-5 sm:grid-cols-2 xl:grid-cols-3">
    {#each visible as item, idx (item.url)}
      <ItemCard {item} index={idx} {dismissable} {onOpen} />
    {/each}
  </div>
{:else}
  <div class="card divide-y divide-line-soft overflow-hidden">
    {#each visible as item (item.url)}
      <ItemRow {item} variant={density.value === "compact" ? "compact" : "list"} {dismissable} {onOpen} />
    {/each}
  </div>
{/if}

<InfiniteScroll {shown} total={items.length} {noun} onMore={() => (shown += PAGE)} />
