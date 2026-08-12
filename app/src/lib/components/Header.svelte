<script lang="ts">
  import { page } from "$app/state";
  import { feed } from "$lib/feed.svelte";
  import { theme } from "$lib/theme.svelte";

  const NAV = [
    { href: "/", label: "Feed" },
    { href: "/internships", label: "Internships" },
    { href: "/settings", label: "Settings" },
  ];

  const themeIcon = $derived(
    theme.value === "light" ? "☀️" : theme.value === "dark" ? "🌙" : "🖥️",
  );
</script>

<header
  class="glass safe-top sticky top-0 z-40 border-b border-zinc-200 dark:border-zinc-800"
>
  <div class="mx-auto flex h-14 w-full max-w-6xl items-center justify-between gap-3 px-4 sm:px-6">
    <a href="/" class="flex shrink-0 items-center gap-2">
      <span
        class="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-600 to-violet-600 text-xs font-bold text-white"
      >
        AF
      </span>
      <span class="font-display hidden text-lg font-bold sm:block">Anti-FOMO</span>
    </a>

    <nav class="flex items-center gap-1">
      {#each NAV as link (link.href)}
        <a
          href={link.href}
          class="rounded-lg px-3 py-1.5 text-sm font-semibold transition-colors {page.url
            .pathname === link.href
            ? 'bg-zinc-900 text-white dark:bg-white dark:text-zinc-900'
            : 'text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800'}"
        >
          {link.label}
        </a>
      {/each}
    </nav>

    <div class="flex shrink-0 items-center gap-1">
      <button
        onclick={() => theme.cycle()}
        title="Theme: {theme.value}"
        aria-label="Switch theme"
        class="flex h-8 w-8 items-center justify-center rounded-lg text-sm transition-colors hover:bg-zinc-100 dark:hover:bg-zinc-800"
      >
        {themeIcon}
      </button>
      <button
        onclick={() => feed.refresh(true)}
        disabled={feed.refreshing}
        title="Refresh all sources"
        aria-label="Refresh"
        class="flex h-8 w-8 items-center justify-center rounded-lg text-sm transition-colors hover:bg-zinc-100 disabled:opacity-50 dark:hover:bg-zinc-800"
      >
        <span class="inline-block {feed.refreshing ? 'animate-spin' : ''}">↻</span>
      </button>
    </div>
  </div>
</header>
