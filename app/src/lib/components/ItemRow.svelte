<script lang="ts">
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, BookmarkCheck, Star, X, iconForType } from "$lib/icons";
  import { splitTitle, tagsFor, typeBadgeClass } from "$lib/item";

  // The row form of a result, for scanning rather than browsing. Two variants
  // share this file because they differ only in what is allowed to survive the
  // squeeze: "compact" keeps the title, source and time, "list" adds the
  // summary and tags. Rows sit inside a divided container, so the row itself
  // draws no border of its own.
  let {
    item,
    onOpen,
    variant = "list",
    selected = false,
    dismissable = true,
  }: {
    item: ScrapedItem;
    onOpen: (item: ScrapedItem) => void;
    variant?: "list" | "compact";
    /** Highlights the row the split-pane detail column is showing. */
    selected?: boolean;
    dismissable?: boolean;
  } = $props();

  let logoFailed = $state(false);

  const logo = $derived(logoFor(item.url));
  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
  const TypeIcon = $derived(iconForType(item.item_type));
  const isTopMatch = $derived((item.relevance_score ?? 0) >= 15);
  const compact = $derived(variant === "compact");
</script>

<!--
  An article rather than a button: the row carries its own save and dismiss
  buttons, which cannot be nested inside another button. The title button is the
  accessible name; the stretched span makes the rest of the row clickable too.
-->
<article
  aria-current={selected ? "true" : undefined}
  class="group relative flex items-start gap-3 px-4 transition-colors
         {compact ? 'py-2' : 'py-3.5'}
         {selected ? 'bg-brand-soft' : 'hover:bg-line-soft'}
         {item.seen && !selected ? 'opacity-65 hover:opacity-100' : ''}"
>
  {#if logo && !logoFailed}
    <img
      src={logo}
      alt=""
      width={compact ? 20 : 36}
      height={compact ? 20 : 36}
      onerror={() => (logoFailed = true)}
      class="shrink-0 rounded-md bg-line-soft object-contain {compact
        ? 'mt-0.5 h-5 w-5 p-0.5'
        : 'h-9 w-9 rounded-lg p-1'}"
    />
  {:else}
    <div
      class="flex shrink-0 items-center justify-center rounded-md bg-line-soft font-bold text-muted {compact
        ? 'mt-0.5 h-5 w-5 text-[10px]'
        : 'h-9 w-9 rounded-lg text-sm'}"
    >
      {parts.primary.charAt(0).toUpperCase()}
    </div>
  {/if}

  <div class="min-w-0 flex-1">
    <div class="flex items-start gap-2">
      <h3 class="min-w-0 flex-1 leading-snug font-semibold {compact ? 'text-sm' : 'text-[15px]'}">
        <button
          onclick={() => onOpen(item)}
          class="line-clamp-2 text-left hover:text-brand {selected ? 'text-brand-soft-fg' : ''}"
        >
          <span class="absolute inset-0" aria-hidden="true"></span>
          {parts.primary}
        </button>
      </h3>

      <!-- Above the stretched link, or the row would swallow these clicks. -->
      <div class="relative z-10 flex shrink-0 items-center gap-1">
        {#if isTopMatch && !compact}
          <span
            class="inline-flex items-center gap-0.5 rounded-full bg-star-soft px-1.5 py-0.5 text-[10px] font-bold text-star"
            title="Matches your field and interests"
          >
            <Star size={9} strokeWidth={3} />
            TOP
          </span>
        {/if}
        <button
          onclick={() => feed.toggleSaved(item)}
          aria-pressed={item.saved}
          aria-label={item.saved ? "Remove from saved" : "Save this item"}
          title={item.saved ? "Remove from saved" : "Save"}
          class="flex h-6 w-6 items-center justify-center rounded-md text-subtle transition-all
                 hover:bg-line hover:text-brand focus-visible:opacity-100
                 {item.saved ? 'text-brand opacity-100' : 'opacity-0 group-hover:opacity-100'}"
        >
          {#if item.saved}
            <BookmarkCheck size={14} strokeWidth={2.5} />
          {:else}
            <Bookmark size={14} />
          {/if}
        </button>
        {#if dismissable}
          <button
            onclick={() => feed.dismiss(item)}
            aria-label="Hide this item"
            title="Not interested"
            class="flex h-6 w-6 items-center justify-center rounded-md text-subtle opacity-0 transition-all
                   hover:bg-line hover:text-foreground group-hover:opacity-100 focus-visible:opacity-100"
          >
            <X size={14} />
          </button>
        {/if}
      </div>
    </div>

    {#if parts.secondary && !compact}
      <p class="line-clamp-1 text-sm font-medium text-muted">{parts.secondary}</p>
    {/if}

    {#if !compact && !parts.secondary && item.content_text}
      <p class="mt-0.5 line-clamp-1 text-sm text-muted">{item.content_text}</p>
    {/if}

    <div
      class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] font-medium text-subtle"
    >
      <span class="inline-flex items-center gap-1">
        <TypeIcon size={11} strokeWidth={2.5} class={typeBadgeClass(item.item_type).split(" ")[1]} />
        {item.source_platform}
      </span>
      <span aria-hidden="true">·</span>
      <span>{timeAgo(item.timestamp)}</span>
      {#if !compact}
        {#each tags as tag (tag)}
          <span class="rounded bg-line-soft px-1.5 py-0.5 text-[10px] text-muted">{tag}</span>
        {/each}
      {/if}
    </div>
  </div>
</article>
