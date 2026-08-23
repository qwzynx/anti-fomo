<script lang="ts">
  import { timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { CornerDownLeft, Search, iconForType } from "$lib/icons";
  import { splitTitle } from "$lib/item";
  import { search } from "$lib/search.svelte";

  // Search for the whole app, opened from the top bar or ⌘K. It reads the same
  // store every page reads, so there is nothing to keep in sync and no page
  // that can be searched from somewhere else and not from here.
  let { onOpen }: { onOpen: (item: ScrapedItem) => void } = $props();

  /** Enough to choose from without the list becoming its own scrolling feed. */
  const LIMIT = 8;

  let input = $state<HTMLInputElement | null>(null);
  let cursor = $state(0);

  // Both lists, because a role that has aged out of the ranked feed is still a
  // role you might be looking for by name.
  const corpus = $derived.by(() => {
    const byUrl = new Map<string, ScrapedItem>();
    for (const item of [...feed.items, ...feed.internships, ...feed.saved]) {
      if (!byUrl.has(item.url)) byUrl.set(item.url, item);
    }
    return [...byUrl.values()];
  });

  const results = $derived.by(() => {
    const needle = search.query.trim().toLowerCase();
    if (needle.length < 2) return [];

    // Title hits before body hits: someone typing "stripe" wants the Stripe
    // posting, not the article that mentions Stripe in passing.
    const title: ScrapedItem[] = [];
    const body: ScrapedItem[] = [];
    for (const item of corpus) {
      if (item.title.toLowerCase().includes(needle)) title.push(item);
      else if (
        (item.content_text ?? "").toLowerCase().includes(needle) ||
        (item.location ?? "").toLowerCase().includes(needle)
      )
        body.push(item);
      if (title.length >= LIMIT) break;
    }
    return [...title, ...body].slice(0, LIMIT);
  });

  // A shrinking result list must never leave the highlight past its end.
  $effect(() => {
    if (cursor >= results.length) cursor = 0;
  });

  $effect(() => {
    if (search.open) input?.focus();
  });

  function choose(item: ScrapedItem) {
    search.hide();
    onOpen(item);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      search.hide();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      cursor = results.length === 0 ? 0 : (cursor + 1) % results.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      cursor = results.length === 0 ? 0 : (cursor - 1 + results.length) % results.length;
    } else if (e.key === "Enter" && results[cursor]) {
      e.preventDefault();
      choose(results[cursor]);
    }
  }
</script>

{#if search.open}
  <!-- The backdrop is a button so a click outside closes, which is what every
       palette on every platform does. Escape is handled on the panel. -->
  <div class="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[12vh]">
    <button
      class="animate-fade-in absolute inset-0 cursor-default bg-foreground/25 backdrop-blur-[2px]"
      aria-label="Close search"
      onclick={() => search.hide()}
    ></button>

    <!-- tabindex so the panel itself can hold focus while the arrow keys walk
         the result list; focus starts in the input either way. -->
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Search"
      tabindex="-1"
      onkeydown={onKeydown}
      class="animate-fade-up relative w-full max-w-2xl overflow-hidden rounded-[20px] border border-line bg-surface shadow-2xl shadow-foreground/10"
    >
      <div class="flex items-center gap-3 border-b border-line-soft px-4">
        <Search size={17} class="shrink-0 text-subtle" />
        <input
          bind:this={input}
          bind:value={search.query}
          type="text"
          placeholder="Search roles, reading and events"
          aria-label="Search everything"
          class="h-14 min-w-0 flex-1 bg-transparent text-[15px] outline-none placeholder:text-subtle"
        />
        <kbd
          class="hidden shrink-0 rounded-md border border-line px-1.5 py-0.5 text-[11px] font-semibold text-subtle sm:block"
        >
          Esc
        </kbd>
      </div>

      {#if results.length > 0}
        <ul class="max-h-[52vh] overflow-y-auto py-1.5">
          {#each results as item, i (item.url)}
            {@const parts = splitTitle(item)}
            {@const TypeIcon = iconForType(item.item_type)}
            <li>
              <button
                onclick={() => choose(item)}
                onmouseenter={() => (cursor = i)}
                class="flex w-full items-center gap-3 px-4 py-2.5 text-left {i === cursor
                  ? 'bg-brand-soft'
                  : ''}"
              >
                <TypeIcon
                  size={15}
                  strokeWidth={2}
                  class="shrink-0 {i === cursor ? 'text-brand-soft-fg' : 'text-subtle'}"
                />
                <span class="min-w-0 flex-1">
                  <span
                    class="block truncate text-sm font-semibold {i === cursor
                      ? 'text-brand-soft-fg'
                      : ''}"
                  >
                    {parts.primary}
                  </span>
                  <span class="block truncate text-xs text-subtle">
                    {parts.secondary ? `${parts.secondary} · ` : ""}{item.source_platform} ·
                    {timeAgo(item.timestamp)}
                  </span>
                </span>
                {#if i === cursor}
                  <CornerDownLeft size={14} class="shrink-0 text-brand-soft-fg" />
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="px-4 py-8 text-center text-sm text-muted">
          {search.query.trim().length < 2
            ? "Type at least two characters."
            : `Nothing matches “${search.query.trim()}”.`}
        </p>
      {/if}
    </div>
  </div>
{/if}
