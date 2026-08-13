<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import { Monitor, Moon, RefreshCw, Sun } from "$lib/icons";
  import { theme } from "$lib/theme.svelte";

  // Phones and small tablets only — md: and up gets the sidebar instead, which
  // already carries the brand, the refresh button and the theme toggle. There
  // are no navigation links here because the bottom tab bar owns that job and
  // duplicating it would waste the top row on a 360px screen.

  const ThemeIcon = $derived(
    theme.value === "light" ? Sun : theme.value === "dark" ? Moon : Monitor,
  );
</script>

<header class="glass safe-top sticky top-0 z-40 border-b border-line md:hidden">
  <div class="flex h-14 w-full items-center justify-between gap-3 px-4">
    <a href="/" class="flex shrink-0 items-center gap-2">
      <span
        class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-600 to-violet-600 text-xs font-bold text-white"
      >
        AF
      </span>
      <span class="font-display text-lg font-bold">Anti-FOMO</span>
    </a>

    <div class="flex shrink-0 items-center gap-1">
      <button
        onclick={() => theme.cycle()}
        title="Theme: {theme.value}"
        aria-label="Switch theme (currently {theme.value})"
        class="flex h-9 w-9 items-center justify-center rounded-lg text-muted transition-colors hover:bg-line-soft hover:text-foreground"
      >
        <ThemeIcon size={17} />
      </button>
      <button
        onclick={() => feed.refresh(true)}
        disabled={feed.refreshing}
        title="Refresh all sources"
        aria-label={feed.refreshing ? "Refreshing sources" : "Refresh all sources"}
        class="flex h-9 w-9 items-center justify-center rounded-lg text-muted transition-colors hover:bg-line-soft hover:text-foreground disabled:opacity-50"
      >
        <RefreshCw size={17} class={feed.refreshing ? "animate-spin" : ""} />
      </button>
    </div>
  </div>
</header>
