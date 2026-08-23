<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import { RefreshCw, Search } from "$lib/icons";
  import { search } from "$lib/search.svelte";

  // Every page opens the same way: its name, one line saying what is in it, and
  // — on a phone, which has no top bar to keep them in — the two controls that
  // belong to the app rather than to the page.
  let {
    title,
    subtitle,
    actions,
  }: {
    title: string;
    subtitle?: string;
    /** Page-specific controls, shown at every width beside the title. */
    actions?: import("svelte").Snippet;
  } = $props();
</script>

<div class="safe-top-mobile flex items-start justify-between gap-4 pt-4 pb-3 sm:pt-6 md:pt-6">
  <div class="min-w-0">
    <h1 class="font-display text-[26px] leading-tight font-bold tracking-tight sm:text-3xl">
      {title}
    </h1>
    {#if subtitle}
      <p class="mt-1 text-sm text-muted">{subtitle}</p>
    {/if}
  </div>

  <div class="flex shrink-0 items-center gap-1.5">
    {#if actions}{@render actions()}{/if}

    <!-- md: and up already has both of these in the top bar. -->
    <button
      onclick={() => search.show()}
      aria-label="Search"
      class="tap h-11 w-11 bg-background md:hidden"
    >
      <Search size={19} />
    </button>
    <button
      onclick={() => feed.refresh(true)}
      disabled={feed.refreshing}
      aria-label={feed.refreshing ? "Refreshing sources" : "Refresh all sources"}
      class="tap h-11 w-11 bg-background disabled:opacity-60 md:hidden"
    >
      <RefreshCw size={19} class={feed.refreshing ? "animate-spin" : ""} />
    </button>
  </div>
</div>
