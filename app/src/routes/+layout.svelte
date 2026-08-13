<script lang="ts">
  import "../app.css";
  import BottomNav from "$lib/components/BottomNav.svelte";
  import Header from "$lib/components/Header.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { feed } from "$lib/feed.svelte";
  import { CircleAlert } from "$lib/icons";
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

<!--
  Two shells in one. Below md the page scrolls under a sticky top bar with the
  tab bar pinned to the bottom; at md and up the sidebar takes the left edge and
  the content column scrolls beside it. Pages render inside the column either
  way, so none of them has to know which shell it is in.
-->
<div class="flex min-h-screen bg-background font-sans text-foreground">
  <Sidebar />

  <div class="flex min-w-0 flex-1 flex-col">
    <Header />

    {#if feed.error}
      <div
        class="flex items-center justify-center gap-2 border-b border-star/30 bg-star-soft px-4 py-2 text-center text-xs font-medium text-star"
        role="status"
      >
        <CircleAlert size={14} class="shrink-0" />
        Showing cached results — last refresh failed ({feed.error})
      </div>
    {/if}

    {@render children()}

    <footer class="mt-auto border-t border-line py-8 text-center text-sm text-subtle">
      <p class="px-6">
        {feed.status?.sources.length ?? 0} sources scraped on this device · nothing leaves your machine
      </p>
    </footer>

    <!-- Clearance for the fixed tab bar, which would otherwise cover the footer. -->
    <div class="safe-bottom h-16 md:hidden" aria-hidden="true"></div>
  </div>
</div>

<BottomNav />
