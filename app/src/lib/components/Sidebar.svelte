<script lang="ts">
  import { page } from "$app/state";
  import { timeAgo } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Monitor, Moon, RefreshCw, Sun } from "$lib/icons";
  import { NAV } from "$lib/nav";
  import { theme } from "$lib/theme.svelte";

  // The desktop shell. Persistent side navigation is what every app in this
  // category settles on once there is more than a page or two of content, and
  // it frees the top of the window for the search and filter bar each page
  // needs. Phones keep the bottom tab bar instead; this is hidden below md.
  //
  // Between md and lg it collapses to an icon rail, because a 240px sidebar in
  // a 768px window leaves the results column too narrow to be worth it.

  const ThemeIcon = $derived(
    theme.value === "light" ? Sun : theme.value === "dark" ? Moon : Monitor,
  );

  /** The number worth showing beside a destination, if any. */
  function countFor(href: string): number | null {
    switch (href) {
      case "/":
        return feed.items.length || null;
      case "/internships":
        return feed.internships.length || null;
      case "/saved":
        return feed.status?.saved_count || null;
      default:
        return null;
    }
  }
</script>

<aside
  class="safe-top sticky top-0 hidden h-screen shrink-0 flex-col border-r border-line bg-surface md:flex md:w-16 lg:w-60"
>
  <a href="/" class="flex h-14 shrink-0 items-center gap-2 px-3 lg:px-4" aria-label="Anti-FOMO home">
    <span
      class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-600 to-violet-600 text-xs font-bold text-white"
    >
      AF
    </span>
    <span class="hidden font-display text-lg font-bold lg:inline">Anti-FOMO</span>
  </a>

  <nav class="flex flex-1 flex-col gap-1 p-2 lg:p-3" aria-label="Main">
    {#each NAV as link (link.href)}
      {@const active = page.url.pathname === link.href}
      {@const count = countFor(link.href)}
      <a
        href={link.href}
        title={link.label}
        aria-current={active ? "page" : undefined}
        class="flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-semibold transition-colors
               {active
          ? 'bg-brand-soft text-brand-soft-fg'
          : 'text-muted hover:bg-line-soft hover:text-foreground'}"
      >
        <link.icon size={19} strokeWidth={active ? 2.5 : 2} class="shrink-0" />
        <span class="hidden lg:inline">{link.label}</span>
        {#if count !== null}
          <span
            class="ml-auto hidden rounded-full px-1.5 py-0.5 text-[10px] font-bold tabular-nums lg:inline
                   {active ? 'bg-brand text-brand-fg' : 'bg-line-soft text-subtle'}"
          >
            {count > 999 ? "999+" : count}
          </span>
        {/if}
      </a>
    {/each}
  </nav>

  <div class="flex shrink-0 flex-col gap-2 border-t border-line p-2 lg:p-3">
    <button
      onclick={() => feed.refresh(true)}
      disabled={feed.refreshing}
      title="Refresh all sources"
      aria-label={feed.refreshing ? "Refreshing sources" : "Refresh all sources"}
      class="flex items-center justify-center gap-2 rounded-xl border border-line px-3 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-brand disabled:opacity-50 lg:justify-start"
    >
      <RefreshCw size={17} class="shrink-0 {feed.refreshing ? 'animate-spin' : ''}" />
      <span class="hidden lg:inline">{feed.refreshing ? "Refreshing…" : "Refresh"}</span>
    </button>

    <button
      onclick={() => theme.cycle()}
      title="Theme: {theme.value}"
      aria-label="Switch theme (currently {theme.value})"
      class="flex items-center justify-center gap-2 rounded-xl px-3 py-2 text-sm font-semibold text-muted transition-colors hover:bg-line-soft hover:text-foreground lg:justify-start"
    >
      <ThemeIcon size={17} class="shrink-0" />
      <span class="hidden capitalize lg:inline">{theme.value}</span>
    </button>

    <p class="hidden px-1 text-[11px] leading-snug text-subtle lg:block">
      {feed.status?.item_count ?? 0} items cached
      <br />
      {feed.status?.last_refresh ? `updated ${timeAgo(feed.status.last_refresh)}` : "not yet synced"}
    </p>
  </div>
</aside>
