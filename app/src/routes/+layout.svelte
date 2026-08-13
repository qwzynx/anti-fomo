<script lang="ts">
  import "../app.css";
  import BottomNav from "$lib/components/BottomNav.svelte";
  import Header from "$lib/components/Header.svelte";
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

<div class="flex min-h-screen flex-col bg-background font-sans text-foreground">
  <Header />

  {#if feed.error}
    <div
      class="flex items-center justify-center gap-2 border-b border-star/30 bg-star-soft px-4 py-2 text-center text-xs font-medium text-star"
      role="status"
    >
      <CircleAlert size={14} />
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
  <div class="safe-bottom h-16 sm:hidden" aria-hidden="true"></div>
</div>

<BottomNav />
