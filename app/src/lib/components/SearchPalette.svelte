<script lang="ts">
  import { goto } from "$app/navigation";
  import { timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { ArrowUpRight, CornerDownLeft, Search, iconForType } from "$lib/icons";
  import { splitTitle } from "$lib/item";
  import * as api from "$lib/api";
  import {
    highlight,
    highlightTerms,
    indexableTerms,
    parseQuery,
    search as find,
    type DescriptionHits,
  } from "$lib/query";
  import { roles } from "$lib/roles.svelte";
  import { search } from "$lib/search.svelte";

  // Search for the whole app, opened from the top bar or ⌘K/Ctrl-K. It reads
  // the same store every page reads and the same matcher Roles uses, so the two
  // never disagree about what "react toronto" finds.
  let { onOpen }: { onOpen: (item: ScrapedItem) => void } = $props();

  /** Per group: enough to choose from without the list becoming its own feed. */
  const GROUP_LIMIT = { roles: 6, other: 3 };
  const QUERY_DEBOUNCE_MS = 80;

  let input = $state<HTMLInputElement | null>(null);
  let cursor = $state(0);
  /** The query the results are for, a beat behind the field. */
  let settled = $state(search.query);
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const q = search.query;
    clearTimeout(timer);
    timer = setTimeout(() => (settled = q), QUERY_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });

  /**
   * Everything that is not a role: the feed's reading and events, and saved
   * items of any kind that have aged out of the ranked lists. Built once per
   * snapshot, so its search index is too.
   */
  const others = $derived.by(() => {
    const seen = new Set(feed.internships.map((i) => i.url));
    const out: ScrapedItem[] = [];
    for (const item of [...feed.items, ...feed.saved]) {
      if (seen.has(item.url)) continue;
      seen.add(item.url);
      out.push(item);
    }
    return out;
  });

  const parsed = $derived(parseQuery(settled));
  const marks = $derived(highlightTerms(parsed));
  const tooShort = $derived(settled.trim().length < 2);

  /** Roles whose fetched description has the words — the same answer Roles uses, so "See all N" lands on N. */
  let described = $state.raw<{ key: string; hits: DescriptionHits } | null>(null);
  const describedKey = $derived(search.open && !tooShort ? indexableTerms(parsed).join(" ") : "");
  $effect(() => {
    const key = describedKey;
    if (!key) return;
    let live = true;
    api.searchDescriptions(key.split(" ")).then(
      (raw) => {
        if (!live) return;
        const hits: DescriptionHits = new Map();
        for (const [term, urls] of Object.entries(raw)) hits.set(term, new Set(urls));
        described = { key, hits };
      },
      () => {},
    );
    return () => {
      live = false;
    };
  });
  const descriptionHits = $derived(described?.key === describedKey ? described.hits : undefined);

  /** Title and employer hits first, then the rest, each in rank order. */
  function ranked(
    list: readonly ScrapedItem[],
    hits?: DescriptionHits,
  ): { all: number; top: ScrapedItem[] } {
    const hit = find(list, parsed, hits);
    const head: ScrapedItem[] = [];
    const rest: ScrapedItem[] = [];
    for (const item of hit.items) (hit.tier.get(item.url) === 2 ? head : rest).push(item);
    return { all: hit.items.length, top: [...head, ...rest] };
  }

  const groups = $derived.by(() => {
    // Only while the palette is up: this is a scan of the whole corpus.
    if (!search.open || tooShort || parsed.empty) return null;
    const r = ranked(feed.internships, descriptionHits);
    const o = ranked(others);
    return {
      roles: r.top.slice(0, GROUP_LIMIT.roles),
      rolesTotal: r.all,
      other: o.top.slice(0, GROUP_LIMIT.other * 2),
    };
  });

  /** Everything the arrow keys walk, in order; the "see all" row is last. */
  const flat = $derived(groups ? [...groups.roles, ...groups.other] : []);
  const seeAllIndex = $derived(groups && groups.rolesTotal > groups.roles.length ? flat.length : -1);
  const total = $derived(flat.length + (seeAllIndex >= 0 ? 1 : 0));

  $effect(() => {
    if (cursor >= total) cursor = 0;
  });

  $effect(() => {
    if (search.open) {
      input?.focus();
      input?.select();
    }
  });

  function choose(item: ScrapedItem) {
    search.hide();
    onOpen(item);
  }

  async function seeAll() {
    const query = search.query.trim();
    search.hide();
    // The stored chips must have been read first, or they would be restored
    // on top of the query after the clear below.
    await roles.init();
    // "See all 46" should land on 46, so the chips Roles had on are cleared
    // (the sort stays: it orders, it does not narrow).
    roles.clear();
    roles.filters.query = query;
    void goto("/internships");
  }

  function activate(i: number) {
    if (i === seeAllIndex) void seeAll();
    else if (flat[i]) choose(flat[i]);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      // Only this layer: a dialog open beneath the palette stays open.
      e.stopPropagation();
      search.hide();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      cursor = total === 0 ? 0 : (cursor + 1) % total;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      cursor = total === 0 ? 0 : (cursor - 1 + total) % total;
    } else if (e.key === "Enter") {
      e.preventDefault();
      // Shift-Enter, or Enter with nothing to pick: the full list on Roles.
      if (e.shiftKey || total === 0) {
        if (!tooShort) void seeAll();
      } else activate(cursor);
    } else if (e.key === "Tab") {
      // The input is the only stop; Tab would otherwise walk into the page behind.
      e.preventDefault();
    }
  }

  const optionId = (i: number) => `palette-option-${i}`;
</script>

{#snippet row(item: ScrapedItem, i: number)}
  {@const parts = splitTitle(item)}
  {@const TypeIcon = iconForType(item.item_type)}
  {@const active = i === cursor}
  <li
    id={optionId(i)}
    role="option"
    aria-selected={active}
    class="flex cursor-pointer items-center gap-3 px-4 py-2.5 {active ? 'bg-brand-soft' : ''}"
    onclick={() => choose(item)}
    onmouseenter={() => (cursor = i)}
    onkeydown={() => {}}
  >
    <TypeIcon
      size={15}
      strokeWidth={2}
      class="shrink-0 {active ? 'text-brand-soft-fg' : 'text-subtle'}"
    />
    <span class="min-w-0 flex-1">
      <span class="block truncate text-sm font-semibold {active ? 'text-brand-soft-fg' : ''}">
        {#each highlight(parts.primary, marks) as run, j (j)}{#if run.hit}<mark
              class="rounded-sm bg-star-soft px-px text-inherit">{run.text}</mark
            >{:else}{run.text}{/if}{/each}
      </span>
      <span class="block truncate text-xs text-subtle">
        {parts.secondary ? `${parts.secondary} · ` : ""}{item.location?.split("|")[0].trim() ||
          item.source_platform} · {timeAgo(item.timestamp)}
      </span>
    </span>
    {#if active}
      <CornerDownLeft size={14} class="shrink-0 text-brand-soft-fg" />
    {/if}
  </li>
{/snippet}

{#if search.open}
  <div class="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[12vh]">
    <button
      class="animate-fade-in absolute inset-0 cursor-default bg-black/40 backdrop-blur-[2px]"
      aria-label="Close search"
      tabindex="-1"
      onclick={() => search.hide()}
    ></button>

    <div
      role="dialog"
      aria-modal="true"
      aria-label="Search"
      tabindex="-1"
      onkeydown={onKeydown}
      class="animate-fade-up relative w-full max-w-2xl overflow-hidden rounded-xl border border-line bg-elevated shadow-2xl shadow-black/25"
    >
      <div class="flex items-center gap-3 border-b border-line-soft px-4">
        <Search size={17} class="shrink-0 text-subtle" />
        <input
          bind:this={input}
          bind:value={search.query}
          type="text"
          role="combobox"
          aria-expanded={total > 0}
          aria-controls="palette-results"
          aria-activedescendant={total > 0 ? optionId(cursor) : undefined}
          aria-autocomplete="list"
          placeholder="Search roles, companies, reading and events"
          aria-label="Search everything"
          autocomplete="off"
          spellcheck="false"
          class="h-14 min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-subtle"
        />
        <kbd
          class="hidden shrink-0 rounded-md border border-line px-1.5 py-0.5 text-[11px] font-semibold text-subtle sm:block"
        >
          Esc
        </kbd>
      </div>

      {#if groups && total > 0}
        <ul id="palette-results" role="listbox" aria-label="Results" class="max-h-[56vh] overflow-y-auto py-1.5">
          {#if groups.roles.length > 0}
            <li role="presentation" class="px-4 pt-1.5 pb-1 text-[11px] font-bold tracking-wider text-subtle uppercase">
              Roles
            </li>
            {#each groups.roles as item, i (item.url)}
              {@render row(item, i)}
            {/each}
          {/if}
          {#if seeAllIndex >= 0}
            <li
              id={optionId(seeAllIndex)}
              role="option"
              aria-selected={cursor === seeAllIndex}
              class="flex cursor-pointer items-center gap-3 px-4 py-2.5 text-sm font-semibold text-brand {cursor ===
              seeAllIndex
                ? 'bg-brand-soft'
                : ''}"
              onclick={() => void seeAll()}
              onmouseenter={() => (cursor = seeAllIndex)}
              onkeydown={() => {}}
            >
              <ArrowUpRight size={15} class="shrink-0" />
              <span class="flex-1">See all {groups.rolesTotal.toLocaleString()} matching roles</span>
              <kbd class="hidden rounded-md border border-line px-1.5 text-[11px] text-subtle sm:block"
                >Shift ↵</kbd
              >
            </li>
          {/if}
          {#if groups.other.length > 0}
            <li role="presentation" class="px-4 pt-3 pb-1 text-[11px] font-bold tracking-wider text-subtle uppercase">
              Reading, events and saved
            </li>
            {#each groups.other as item, k (item.url)}
              {@render row(item, groups.roles.length + k)}
            {/each}
          {/if}
        </ul>
      {:else}
        <div class="px-4 py-8 text-center text-sm text-muted">
          {#if tooShort}
            Type at least two characters.
            <p class="mt-2 text-xs text-subtle">
              Every word must match · <span class="font-mono">"exact phrase"</span> ·
              <span class="font-mono">-exclude</span> · <span class="font-mono">company:name</span>
            </p>
          {:else}
            Nothing matches “{settled.trim()}”.
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}
