<script lang="ts">
  import "../app.css";
  import type { ScrapedItem } from "$lib/api";
  import BottomNav from "$lib/components/BottomNav.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import SearchPalette from "$lib/components/SearchPalette.svelte";
  import SkillsWizard from "$lib/components/SkillsWizard.svelte";
  import TopBar from "$lib/components/TopBar.svelte";
  import { feed } from "$lib/feed.svelte";
  import { CircleAlert } from "$lib/icons";
  import { search } from "$lib/search.svelte";
  import { theme } from "$lib/theme.svelte";

  let { children } = $props();

  /** What the palette opened, mounted here so search works from any page. */
  let found = $state<ScrapedItem | null>(null);

  $effect(() => {
    feed.init();
    return theme.watchSystem();
  });

  /** ⌘K / Ctrl-K anywhere. Ignored while typing, so it cannot eat a keystroke. */
  function onKeydown(e: KeyboardEvent) {
    if (e.key.toLowerCase() !== "k" || !(e.metaKey || e.ctrlKey)) return;
    const el = e.target as HTMLElement | null;
    if (el?.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el?.tagName ?? "")) return;
    e.preventDefault();
    search.toggle();
  }
</script>

<svelte:head>
  <title>Anti-FOMO — Your Student Hub</title>
</svelte:head>

<svelte:window onkeydown={onKeydown} />

<!--
  One shell at every width. The top bar sits above the content at md: and up and
  is simply not rendered below it, where the tab bar owns navigation and each
  page carries its own title row — so the two navigations are never on screen
  together and the content column is never squeezed by a rail.
-->
<div class="flex min-h-screen flex-col bg-background font-sans text-foreground">
  <TopBar />

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

<BottomNav />

<!-- Mounted once here rather than per page: search, the settings button, the
     Today nudge and any job detail all open the same things through the store. -->
<SearchPalette onOpen={(item) => (found = item)} />
<ItemModal item={found} onClose={() => (found = null)} />
<SkillsWizard />
