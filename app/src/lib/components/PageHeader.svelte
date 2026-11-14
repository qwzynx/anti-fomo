<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import { RefreshCw, Search, Square } from "$lib/icons";
  import { search } from "$lib/search.svelte";

  // The bar across the top of every view: what you are looking at, how much of
  // it there is, and the view's own controls. It is chrome, not a page title —
  // one line, fixed, with the content scrolling in a pane beneath it.
  //
  // On a phone there is no sidebar, so search and refresh live here too.
  let {
    title,
    subtitle,
    actions,
    children,
  }: {
    title: string;
    subtitle?: string;
    /** Controls on the right edge. */
    actions?: import("svelte").Snippet;
    /** Controls that fill the middle, e.g. a search field. */
    children?: import("svelte").Snippet;
  } = $props();
</script>

<header class="toolbar">
  <div class="flex min-w-0 items-baseline gap-2.5 {children ? 'shrink-0' : 'flex-1'}">
    <h1 class="shrink-0 text-[15px] font-semibold">{title}</h1>
    {#if subtitle}
      <p class="hidden min-w-0 truncate text-[13px] text-subtle sm:block">{subtitle}</p>
    {/if}
  </div>

  {#if children}
    <div class="flex min-w-0 flex-1 items-center gap-2">{@render children()}</div>
  {/if}

  <div class="flex shrink-0 items-center gap-1">
    {#if actions}{@render actions()}{/if}

    <!-- md: and up has both of these in the sidebar. -->
    <button onclick={() => search.show()} aria-label="Search" class="tool h-9 w-9 md:hidden">
      <Search size={17} />
    </button>
    {#if feed.refreshing}
      <button
        onclick={() => feed.stop()}
        disabled={feed.phase === "stopping"}
        aria-label="Stop scraping"
        class="tool h-9 w-9 md:hidden"
      >
        <Square size={14} class="fill-current" />
      </button>
    {:else}
      <button
        onclick={() => feed.refresh(true)}
        aria-label="Refresh all sources"
        class="tool h-9 w-9 md:hidden"
      >
        <RefreshCw size={16} />
      </button>
    {/if}
  </div>
</header>
