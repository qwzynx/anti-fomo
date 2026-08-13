<script lang="ts">
  // With ~1,600 items cached, rendering the whole filtered list at once costs a
  // visible stall on a phone. Pages reveal a slice at a time and mount this at
  // the bottom; it loads the next slice as the sentinel comes into view.
  let {
    shown,
    total,
    onMore,
    noun = "results",
  }: {
    shown: number;
    total: number;
    onMore: () => void;
    /** What the counter calls the things being listed. */
    noun?: string;
  } = $props();

  let sentinel = $state<HTMLElement | null>(null);

  const hasMore = $derived(shown < total);

  $effect(() => {
    // `shown` is read so the observer is rebuilt after each page. Re-observing
    // an element that is already in view fires again immediately, which is what
    // keeps loading going when a page of results is shorter than the root
    // margin and would otherwise never re-trigger the callback.
    void shown;
    if (!sentinel || !hasMore) return;

    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) onMore();
      },
      // Start the next page before the user reaches the end of this one.
      { rootMargin: "800px" },
    );
    io.observe(sentinel);
    return () => io.disconnect();
  });
</script>

<div bind:this={sentinel} class="flex flex-col items-center gap-3 py-8">
  {#if hasMore}
    <!-- The observer normally gets here first. This stays for keyboard users
         and for the case where it doesn't. -->
    <button
      onclick={onMore}
      class="rounded-xl border border-line px-4 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-brand"
    >
      Show more
    </button>
    <p class="text-xs text-subtle tabular-nums">
      {shown} of {total}
      {noun}
    </p>
  {:else if total > 0}
    <p class="text-xs text-subtle tabular-nums">
      That's all {total}
      {noun}.
    </p>
  {/if}
</div>
