<script lang="ts">
  import "../app.css";
  import Header from "$lib/components/Header.svelte";
  import { feed } from "$lib/feed.svelte";
  import { theme } from "$lib/theme.svelte";

  let { children } = $props();

  $effect(() => {
    feed.init();
    return theme.watchSystem();
  });
</script>

<svelte:head>
  <title>Anti-FOMO — Your Student Hub</title>
</svelte:head>

<div
  class="flex min-h-screen flex-col bg-zinc-50 font-sans text-zinc-900 dark:bg-black dark:text-zinc-100"
>
  <Header />

  {#if feed.error}
    <div
      class="border-b border-amber-200 bg-amber-50 px-4 py-2 text-center text-xs font-medium text-amber-800 dark:border-amber-900/50 dark:bg-amber-950/40 dark:text-amber-300"
    >
      Showing cached results — last refresh failed ({feed.error})
    </div>
  {/if}

  {@render children()}

  <footer
    class="safe-bottom mt-auto border-t border-zinc-200 py-8 text-center text-sm text-zinc-500 dark:border-zinc-800"
  >
    <p class="px-6">
      Aggregated from Hacker News, Phoronix, TLDR, Daily.dev, Lassonde, Luma, Levels.fyi, Pitt CSC
      &amp; Simplify.
    </p>
  </footer>
</div>
