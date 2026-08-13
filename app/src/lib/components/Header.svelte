<script lang="ts">
  import { page } from "$app/state";
  import { feed } from "$lib/feed.svelte";
  import { Monitor, Moon, RefreshCw, Sun } from "$lib/icons";
  import { NAV } from "$lib/nav";
  import { theme } from "$lib/theme.svelte";

  const ThemeIcon = $derived(
    theme.value === "light" ? Sun : theme.value === "dark" ? Moon : Monitor,
  );
</script>

<header class="glass safe-top sticky top-0 z-40 border-b border-line">
  <div class="mx-auto flex h-14 w-full max-w-6xl items-center justify-between gap-3 px-4 sm:px-6">
    <a href="/" class="flex shrink-0 items-center gap-2">
      <span
        class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-600 to-violet-600 text-xs font-bold text-white"
      >
        AF
      </span>
      <span class="font-display text-lg font-bold">Anti-FOMO</span>
    </a>

    <!-- Phones get the bottom tab bar instead; duplicating nav in both places
         would waste the top row's width on a 360px screen. -->
    <nav class="hidden items-center gap-1 sm:flex">
      {#each NAV as link (link.href)}
        <a
          href={link.href}
          aria-current={page.url.pathname === link.href ? "page" : undefined}
          class="rounded-lg px-3 py-1.5 text-sm font-semibold transition-colors {page.url
            .pathname === link.href
            ? 'bg-foreground text-background'
            : 'text-muted hover:bg-line-soft hover:text-foreground'}"
        >
          {link.label}
        </a>
      {/each}
    </nav>

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
