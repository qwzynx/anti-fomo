<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import ItemDetail from "./ItemDetail.svelte";

  // Dialog chrome only: the backdrop, the scroll lock, focus management and the
  // Escape key. Everything inside comes from ItemDetail, which the jobs page
  // renders directly in its right-hand column instead.
  let { item, onClose }: { item: ScrapedItem | null; onClose: () => void } = $props();

  let panel = $state<HTMLElement | null>(null);
  let titleId = $derived(item ? `modal-title-${encodeURIComponent(item.url)}` : undefined);

  // The backdrop scroll-locks the page underneath while open.
  $effect(() => {
    if (!item) return;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = "";
    };
  });

  // Move focus into the dialog on open and hand it back on close, so keyboard
  // and screen-reader users aren't dropped at the top of the document.
  $effect(() => {
    if (!item || !panel) return;
    const previous = document.activeElement as HTMLElement | null;
    panel.focus();
    return () => previous?.focus?.();
  });

  /** Everything focusable inside the panel, in document order. */
  function focusables(): HTMLElement[] {
    if (!panel) return [];
    return [
      ...panel.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => el.offsetParent !== null);
  }

  /** Keeps Tab cycling inside the dialog rather than escaping to the page. */
  function trapFocus(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const nodes = focusables();
    if (nodes.length === 0) return;

    const first = nodes[0];
    const last = nodes[nodes.length - 1];
    const active = document.activeElement;

    if (e.shiftKey && (active === first || active === panel)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (!item) return;
    if (e.key === "Escape") onClose();
    else trapFocus(e);
  }}
/>

{#if item}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
    onclick={onClose}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      bind:this={panel}
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
      class="animate-fade-up safe-bottom card flex max-h-[88vh] w-full max-w-xl flex-col overflow-hidden rounded-t-3xl bg-elevated shadow-2xl focus:outline-none sm:rounded-3xl"
    >
      <ItemDetail {item} {onClose} {titleId} />
    </div>
  </div>
{/if}
