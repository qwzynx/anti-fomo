<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { timeAgo } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import {
    Bookmark,
    Briefcase,
    FileText,
    Hash,
    Home,
    ListChecks,
    Monitor,
    Moon,
    RefreshCw,
    Search,
    Settings,
    Sparkles,
    Square,
    Sun,
    type IconComponent,
  } from "$lib/icons";
  import { modKey } from "$lib/platform";
  import { roles } from "$lib/roles.svelte";
  import { search } from "$lib/search.svelte";
  import { theme } from "$lib/theme.svelte";

  // The app's chrome on a desktop: where you are, how much is there, and the
  // searches you come back to — always on screen, the way a mail client's
  // folders are, rather than a top bar of three tabs and a page that scrolls
  // away underneath it.

  type Link = { href: string; label: string; icon: IconComponent; count?: number | null; match?: (p: string, q: URLSearchParams) => boolean };

  const views = $derived<Link[]>([
    { href: "/", label: "Today", icon: Home, match: (p) => p === "/" },
    {
      href: "/internships",
      label: "Roles",
      icon: Briefcase,
      count: feed.internships.length || null,
      match: (p) => p.startsWith("/internships") && !roles.current(),
    },
    {
      href: "/saved",
      label: "Saved",
      icon: Bookmark,
      count: feed.status?.saved_count || null,
      match: (p, q) => p.startsWith("/saved") && q.get("stage") !== "active",
    },
    {
      href: "/saved?stage=active",
      label: "Applications",
      icon: ListChecks,
      count: feed.activeApplications || null,
      match: (p, q) => p.startsWith("/saved") && q.get("stage") === "active",
    },
  ]);

  const tools = $derived<Link[]>([
    { href: "/resume", label: "Résumé", icon: FileText, match: (p) => p.startsWith("/resume") },
    { href: "/settings", label: "Settings", icon: Settings, match: (p) => p.startsWith("/settings") },
  ]);

  const path = $derived(page.url.pathname);
  const params = $derived(page.url.searchParams);
  const currentSearch = $derived(path.startsWith("/internships") && roles.ready ? roles.current() : null);

  const ThemeIcon = $derived(theme.value === "light" ? Sun : theme.value === "dark" ? Moon : Monitor);

  const synced = $derived(
    feed.refreshing
      ? feed.phase === "stopping"
        ? "Stopping…"
        : "Syncing…"
      : feed.status?.last_refresh
        ? `Synced ${timeAgo(feed.status.last_refresh)}`
        : "Never synced",
  );

  function format(n: number): string {
    return n > 9999 ? `${Math.round(n / 1000)}k` : n.toLocaleString();
  }

  async function openSearch(id: string) {
    await roles.apply(id);
    if (!path.startsWith("/internships")) await goto("/internships");
  }
</script>

{#snippet item(link: Link)}
  {@const active = link.match ? link.match(path, params) : path === link.href}
  <a
    href={link.href}
    aria-current={active ? "page" : undefined}
    class="group flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] transition-colors
           {active
      ? 'bg-hover font-semibold text-foreground'
      : 'font-medium text-muted hover:bg-hover hover:text-foreground'}"
  >
    <link.icon
      size={15}
      strokeWidth={active ? 2.25 : 2}
      class="shrink-0 {active ? 'text-brand' : 'text-subtle group-hover:text-muted'}"
    />
    <span class="min-w-0 flex-1 truncate">{link.label}</span>
    {#if link.count}
      <span class="shrink-0 text-[11px] font-medium text-subtle tabular-nums">{format(link.count)}</span>
    {/if}
  </a>
{/snippet}

<aside
  class="hidden w-[232px] shrink-0 flex-col border-r border-line bg-sidebar md:flex"
  aria-label="Sidebar"
>
  <div class="flex h-[52px] shrink-0 items-center gap-2.5 px-4">
    <span class="flex h-6 w-6 items-center justify-center rounded-md bg-brand text-brand-fg">
      <Sparkles size={13} strokeWidth={2.5} />
    </span>
    <span class="text-[14px] font-semibold tracking-tight">Anti-FOMO</span>
  </div>

  <div class="px-3 pb-2">
    <button
      onclick={() => search.show()}
      class="flex h-8 w-full items-center gap-2 rounded-md border border-line bg-background px-2 text-left text-[13px] text-subtle transition-colors hover:border-subtle/50 hover:text-muted"
    >
      <Search size={14} class="shrink-0" />
      <span class="flex-1">Search</span>
      <span class="kbd">{modKey}K</span>
    </button>
  </div>

  <nav class="pane flex-1 px-3 pt-1" aria-label="Main">
    <div class="flex flex-col gap-px">
      {#each views as link (link.href)}
        {@render item(link)}
      {/each}
    </div>

    {#if roles.saved.length > 0}
      <p class="eyebrow mt-5 mb-1.5 px-2">Saved searches</p>
      <div class="flex flex-col gap-px">
        {#each roles.saved as s (s.id)}
          {@const active = currentSearch?.id === s.id}
          <button
            onclick={() => void openSearch(s.id)}
            aria-current={active ? "page" : undefined}
            class="group flex h-8 items-center gap-2.5 rounded-md px-2 text-left text-[13px] transition-colors
                   {active ? 'bg-hover font-semibold text-foreground' : 'font-medium text-muted hover:bg-hover hover:text-foreground'}"
          >
            <Hash size={14} class="shrink-0 {active ? 'text-brand' : 'text-subtle'}" />
            <span class="min-w-0 flex-1 truncate">{s.name}</span>
          </button>
        {/each}
      </div>
    {/if}

    <p class="eyebrow mt-5 mb-1.5 px-2">Tools</p>
    <div class="flex flex-col gap-px">
      {#each tools as link (link.href)}
        {@render item(link)}
      {/each}
    </div>
  </nav>

  <div class="flex shrink-0 items-center gap-1 border-t border-line px-3 py-2.5">
    {#if feed.refreshing}
      <button
        onclick={() => feed.stop()}
        disabled={feed.phase === "stopping"}
        title="Stop — what was found stays"
        class="tool min-w-0 flex-1 justify-start"
      >
        <Square size={11} class="shrink-0 fill-current" />
        <span class="truncate">{synced}</span>
      </button>
    {:else}
      <button
        onclick={() => feed.refresh(true)}
        title="Refresh all sources"
        class="tool min-w-0 flex-1 justify-start"
      >
        <span
          class="h-1.5 w-1.5 shrink-0 rounded-full {feed.status?.stale ? 'bg-star' : 'bg-job'}"
          aria-hidden="true"
        ></span>
        <span class="truncate text-[12px]">{synced}</span>
        <RefreshCw size={12} class="ml-auto shrink-0" />
      </button>
    {/if}
    <button
      onclick={() => theme.cycle()}
      aria-label="Switch theme (currently {theme.value})"
      title="Theme: {theme.value}"
      class="tool"
    >
      <ThemeIcon size={14} />
    </button>
  </div>
</aside>
