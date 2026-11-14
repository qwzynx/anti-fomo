<script lang="ts">
  import "../app.css";
  import { afterNavigate } from "$app/navigation";
  import type { ScrapedItem } from "$lib/api";
  import BottomNav from "$lib/components/BottomNav.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import ScanBar from "$lib/components/ScanBar.svelte";
  import SearchPalette from "$lib/components/SearchPalette.svelte";
  import SkillsWizard from "$lib/components/SkillsWizard.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { installErrorHandlers } from "$lib/errors";
  import { feed } from "$lib/feed.svelte";
  import { roles } from "$lib/roles.svelte";
  import { CircleAlert, X } from "$lib/icons";
  import { search } from "$lib/search.svelte";
  import { theme } from "$lib/theme.svelte";

  let { children } = $props();

  /** What the palette opened, mounted here so search works from any page. */
  let found = $state<ScrapedItem | null>(null);

  /**
   * The setup form is up: a first launch (nothing is fetched until it is
   * answered), an existing install being offered it once, or Settings having
   * raised it again. `unknown` is none of these, so a returning reader never
   * sees it flash before Rust has said which they are.
   */
  const setup = $derived(
    feed.onboarding === "needed" ||
      feed.onboarding === "offered" ||
      (feed.setupOpen && feed.onboarding === "done"),
  );

  $effect(() => {
    const uninstall = installErrorHandlers();
    feed.init();
    // Before anything can hand Roles a query (the palette's "See all"): the
    // stored chips must land first, or they would land on top of it.
    void roles.init();
    const unwatch = theme.watchSystem();
    return () => {
      uninstall();
      unwatch?.();
    };
  });

  // A new page is a list at its top again, so an update no longer needs to wait.
  afterNavigate(() => feed.noteNavigated());

  /** ⌘K / Ctrl-K anywhere. Ignored while typing, so it cannot eat a keystroke. */
  function onKeydown(e: KeyboardEvent) {
    // Search sits outside the inert shell, so the shortcut has to be refused here too.
    if (setup) return;
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
  An app, not a page. The shell is exactly the window: a sidebar at md: and up,
  and beside it a column whose views each own their scrolling — a toolbar that
  stays put over panes that scroll beneath it. Nothing scrolls the window, so
  there is no footer to reach and no title that slides away.

  Phones have no sidebar; the tab bar below carries navigation, and the column
  leaves room for it.

  `inert` while the setup form is up: it covers the screen, and without this
  Tab would walk straight through it into a page that is not there yet.
-->
<div
  class="flex h-dvh overflow-hidden bg-background font-sans text-[13px] text-foreground"
  inert={setup}
>
  <Sidebar />

  <div
    class="flex min-w-0 flex-1 flex-col pt-[env(safe-area-inset-top)] pb-[calc(3.75rem+env(safe-area-inset-bottom))] md:pt-0 md:pb-0"
  >
    <ScanBar />

    {#if feed.error}
      <div
        class="flex shrink-0 items-center gap-2 border-b border-star/30 bg-star-soft px-4 py-1.5 text-[12px] font-medium text-star"
        role="status"
      >
        <CircleAlert size={13} class="shrink-0" />
        <span class="min-w-0 flex-1 truncate"
          >Showing cached results — the last refresh failed ({feed.error})</span
        >
        <button
          onclick={() => feed.clearError()}
          aria-label="Dismiss"
          class="flex h-6 w-6 shrink-0 items-center justify-center rounded-md hover:bg-star/10"
        >
          <X size={13} />
        </button>
      </div>
    {/if}

    <main class="flex min-h-0 flex-1 flex-col">
      {@render children()}
    </main>
  </div>
</div>

<!-- Fixed-position, so wrapping it changes nothing about layout; it only lets the form's
     `inert` reach the tab bar, which lives outside the shell. -->
<div inert={setup}>
  <BottomNav />
</div>

<!-- Mounted once here rather than per page: search, the settings button, the
     Today nudge and any job detail all open the same things through the store. -->
<SearchPalette onOpen={(item) => (found = item)} />
<ItemModal item={found} onClose={() => (found = null)} />
<SkillsWizard />
<ConfirmDialog />
<Toaster />

{#if setup}
  <Onboarding />
{/if}
