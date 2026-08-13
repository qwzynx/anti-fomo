<script lang="ts">
  import type { Snippet } from "svelte";
  import { ListFilter } from "$lib/icons";

  // The desktop filter bar is a row of selects, which is unusable at 360px.
  // On phones the same controls live in a bottom sheet behind one button.
  let {
    activeCount = 0,
    onReset,
    children,
  }: { activeCount?: number; onReset?: () => void; children: Snippet } = $props();

  let open = $state(false);

  $effect(() => {
    if (!open) return;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = "";
    };
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (open && e.key === "Escape") open = false;
  }}
/>

<button
  onclick={() => (open = true)}
  class="control control-focus flex w-full items-center justify-center gap-2 py-2.5 font-semibold"
>
  <ListFilter size={16} />
  <span>Filters</span>
  {#if activeCount > 0}
    <span class="rounded-full bg-brand px-2 py-0.5 text-[11px] font-bold text-brand-fg">
      {activeCount}
    </span>
  {/if}
</button>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <div
    class="animate-fade-in fixed inset-0 z-50 flex items-end bg-black/50 backdrop-blur-sm"
    onclick={() => (open = false)}
    role="dialog"
    aria-modal="true"
    aria-label="Filters"
    tabindex="-1"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      onclick={(e) => e.stopPropagation()}
      class="animate-slide-up safe-bottom max-h-[85vh] w-full overflow-y-auto rounded-t-3xl border-t border-line bg-elevated p-5"
    >
      <div class="mx-auto mb-4 h-1 w-10 rounded-full bg-line"></div>

      <div class="mb-4 flex items-center justify-between">
        <h3 class="text-lg font-bold">Filters</h3>
        {#if onReset}
          <button
            onclick={onReset}
            class="text-xs font-semibold text-brand"
          >
            Reset all
          </button>
        {/if}
      </div>

      <div class="flex flex-col gap-4">
        {@render children()}
      </div>

      <button
        onclick={() => (open = false)}
        class="mt-6 w-full rounded-xl bg-brand py-3 font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
      >
        Show results
      </button>
    </div>
  </div>
{/if}
