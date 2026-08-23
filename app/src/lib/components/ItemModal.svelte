<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import ItemDetail from "./ItemDetail.svelte";
  import Modal from "./Modal.svelte";

  // Binds one item to the shared dialog chrome. Everything visible comes from
  // ItemDetail, which the jobs page renders directly in its right-hand column
  // instead; everything structural comes from Modal.
  let { item, onClose }: { item: ScrapedItem | null; onClose: () => void } = $props();

  let titleId = $derived(item ? `modal-title-${encodeURIComponent(item.url)}` : undefined);
</script>

<Modal open={item !== null} {onClose} {titleId}>
  {#if item}
    <ItemDetail {item} {onClose} {titleId} />
  {/if}
</Modal>
