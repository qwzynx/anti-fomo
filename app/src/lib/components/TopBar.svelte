<script lang="ts">
  import { page } from "$app/state";
  import { timeAgo } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Monitor, Moon, RefreshCw, Search, Sparkles, Sun } from "$lib/icons";
  import { NAV, TOOLS } from "$lib/nav";
  import { search } from "$lib/search.svelte";
  import { theme } from "$lib/theme.svelte";

  // Desktop chrome, replacing the 240px left rail.
  //
  // The rail cost a fifth of the window on every page to show three links that
  // never change, and it pushed each page into rebuilding its own search box in
  // whatever space was left. One 58px bar carries the same three destinations
  // as a segmented control, gives search a permanent home in the middle, and
  // hands the width back to the results.
  //
  // Phones get the tab bar instead; the two navigations are never both mounted.

  const ThemeIcon = $derived(
    theme.value === "light" ? Sun : theme.value === "dark" ? Moon : Monitor,
  );

  /** The number worth showing beside a destination, if any. */
  function countFor(href: string): number | null {
    switch (href) {
      case "/internships":
        return feed.internships.length || null;
      case "/saved":
        return feed.status?.saved_count || null;
      default:
        return null;
    }
  }

  const synced = $derived(
    feed.refreshing
      ? "Syncing…"
      : feed.status?.last_refresh
        ? `Synced ${timeAgo(feed.status.last_refresh)}`
        : "Never synced",
  );
</script>

<header
  class="safe-top sticky top-0 z-40 hidden shrink-0 border-b border-line bg-surface md:block"
>
  <div class="flex h-[58px] items-center gap-4 px-4 lg:gap-6 lg:px-5">
    <a href="/" class="flex shrink-0 items-center gap-2.5" aria-label="Anti-FOMO home">
      <span
        class="flex h-[30px] w-[30px] items-center justify-center rounded-[9px] bg-brand text-brand-fg"
      >
        <Sparkles size={16} strokeWidth={2.5} />
      </span>
      <span class="hidden font-display text-base font-bold tracking-tight lg:inline">
        Anti-FOMO
      </span>
    </a>

    <!-- Three places you go. Settings is a tool, so it sits with the tools. -->
    <nav class="flex shrink-0 items-center gap-[3px] rounded-xl bg-line-soft p-[3px]" aria-label="Main">
      {#each NAV as link (link.href)}
        {@const active = page.url.pathname === link.href}
        {@const count = countFor(link.href)}
        <a
          href={link.href}
          aria-current={active ? "page" : undefined}
          class="flex h-[34px] items-center gap-2 rounded-[9px] border px-3 text-[13px] transition-colors lg:px-[15px]
                 {active
            ? 'border-line bg-surface font-semibold text-brand-soft-fg'
            : 'border-transparent font-medium text-muted hover:text-foreground'}"
        >
          <link.icon size={15} strokeWidth={active ? 2.5 : 2} class="shrink-0" />
          {link.label}
          {#if count !== null}
            <span
              class="rounded-full px-1.5 text-[11px] font-bold tabular-nums
                     {active ? 'bg-brand-soft text-brand-soft-fg' : 'bg-line text-subtle'}"
            >
              {count > 999 ? "999+" : count}
            </span>
          {/if}
        </a>
      {/each}
    </nav>

    <!-- Search is app chrome now, not a control each page rebuilds for itself. -->
    <div class="flex min-w-0 flex-1 justify-center">
      <button
        onclick={() => search.show()}
        class="flex h-9 w-full max-w-[420px] items-center gap-2.5 rounded-xl border border-line bg-background px-3 text-left transition-colors hover:border-subtle"
      >
        <Search size={15} class="shrink-0 text-subtle" />
        <span class="min-w-0 flex-1 truncate text-[13px] text-subtle">
          Search roles, reading and events
        </span>
        <kbd
          class="hidden shrink-0 rounded-md border border-line bg-surface px-1.5 text-[11px] font-semibold text-subtle lg:block"
        >
          ⌘K
        </kbd>
      </button>
    </div>

    <div class="flex shrink-0 items-center gap-1.5">
      <!-- The rail's status footer, reduced to the part you would act on. -->
      <button
        onclick={() => feed.refresh(true)}
        disabled={feed.refreshing}
        aria-label={feed.refreshing ? "Refreshing sources" : "Refresh all sources"}
        title="Refresh all sources"
        class="flex h-[34px] items-center gap-2 rounded-[10px] border border-line px-3 text-xs font-semibold text-muted transition-colors hover:border-subtle disabled:opacity-60"
      >
        <span
          class="h-1.5 w-1.5 shrink-0 rounded-full {feed.status?.stale ? 'bg-star' : 'bg-job'}"
          aria-hidden="true"
        ></span>
        <span class="hidden lg:inline">{synced}</span>
        <RefreshCw size={13} class="shrink-0 {feed.refreshing ? 'animate-spin' : ''}" />
      </button>

      <button
        onclick={() => theme.cycle()}
        aria-label="Switch theme (currently {theme.value})"
        title="Theme: {theme.value}"
        class="tap h-[34px] w-[34px]"
      >
        <ThemeIcon size={17} />
      </button>

      {#each TOOLS as tool (tool.href)}
        {@const here = page.url.pathname.startsWith(tool.href)}
        <a
          href={tool.href}
          aria-label={tool.label}
          aria-current={here ? "page" : undefined}
          title={tool.label}
          class="tap h-[34px] w-[34px] {here ? 'bg-brand-soft text-brand-soft-fg' : ''}"
        >
          <tool.icon size={17} />
        </a>
      {/each}
    </div>
  </div>
</header>
