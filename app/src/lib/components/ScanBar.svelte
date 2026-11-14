<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import { ChevronUp, LoaderCircle, Square } from "$lib/icons";

  // The one place a running scrape is visible and stoppable from every page.
  // It sits under the top bar rather than in it so it is there on a phone too,
  // where the top bar is not rendered at all.
  //
  // While it runs the lists fill in behind it: results are reloaded every few
  // seconds, and if the reader has scrolled down the list the update waits
  // behind "Show new results" rather than moving what they are looking at.

  const done = $derived(feed.progress.length);
  const stopping = $derived(feed.phase === "stopping");
  const fraction = $derived(feed.planned > 0 ? Math.min(1, done / feed.planned) : 0);

  const label = $derived.by(() => {
    if (stopping) return "Stopping — keeping what was found";
    if (feed.phase === "details") return "Reading job descriptions";
    if (feed.planned > 0) return `Scraping ${done} of ${feed.planned} sources`;
    return "Starting";
  });
</script>

{#if feed.refreshing}
  <div class="border-b border-line bg-surface" role="status" aria-live="polite">
    <div class="flex items-center gap-3 px-4 py-2 text-xs lg:px-5">
      <LoaderCircle size={14} class="shrink-0 animate-spin text-brand" />
      <span class="min-w-0 truncate font-semibold">{label}</span>
      {#if feed.phase === "scraping" && feed.scanRows > 0}
        <span class="hidden shrink-0 text-subtle sm:inline">
          · {feed.scanRows.toLocaleString()} postings so far
        </span>
      {/if}
      <span class="ml-auto flex shrink-0 items-center gap-2">
        {#if feed.pending}
          <button
            type="button"
            onclick={() => feed.applyPending()}
            class="rounded-lg bg-brand-soft px-2.5 py-1 font-semibold text-brand-soft-fg transition-colors hover:bg-brand hover:text-brand-fg"
          >
            Show new results
          </button>
        {/if}
        <button
          type="button"
          onclick={() => feed.stop()}
          disabled={stopping}
          class="inline-flex items-center gap-1.5 rounded-lg border border-line px-2.5 py-1 font-semibold text-muted transition-colors hover:border-subtle hover:text-foreground disabled:opacity-50"
        >
          <Square size={11} class="fill-current" />
          {stopping ? "Stopping…" : "Stop"}
        </button>
      </span>
    </div>
    <div class="h-0.5 bg-line-soft" aria-hidden="true">
      <div
        class="h-full bg-brand transition-[width] duration-300"
        style:width="{fraction * 100}%"
      ></div>
    </div>
  </div>
{:else if feed.pending}
  <div class="border-b border-line bg-surface" role="status">
    <div class="flex items-center justify-between gap-3 px-4 py-2 text-xs lg:px-5">
      <span class="font-semibold">New results are ready</span>
      <button
        type="button"
        onclick={() => feed.applyPending()}
        class="rounded-lg bg-brand-soft px-2.5 py-1 font-semibold text-brand-soft-fg transition-colors hover:bg-brand hover:text-brand-fg"
      >
        Show new results
      </button>
    </div>
  </div>
{/if}

<!--
  The bar above scrolls away with the page, and "pending" only ever turns on
  once the reader has scrolled down a list — so the button in it was off-screen
  exactly when it mattered. This copy floats over the content instead.
-->
{#if feed.pending}
  <button
    type="button"
    onclick={() => {
      feed.applyPending();
      window.scrollTo({ top: 0, behavior: "smooth" });
    }}
    class="animate-fade-up fixed top-[calc(env(safe-area-inset-top)+0.75rem)] left-1/2 z-30 inline-flex -translate-x-1/2 items-center gap-1.5 rounded-full bg-brand px-4 py-2 text-[13px] font-semibold text-brand-fg shadow-lg shadow-brand/30 transition-colors hover:bg-brand-hover md:top-[70px]"
  >
    <ChevronUp size={14} strokeWidth={2.5} />
    Show new results
  </button>
{/if}
