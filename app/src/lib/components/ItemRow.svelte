<script lang="ts">
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, BookmarkCheck, X, iconForType } from "$lib/icons";
  import { splitTitle, typeBadgeClass } from "$lib/item";
  import { skillMatch } from "$lib/skills";

  // One result, everywhere a result is listed. The redesign has no card grid
  // and no density switch, so this is the only shape a result takes — which is
  // why it is worth the two variants rather than two components:
  //
  //   band  — a page band or the saved list: a metadata column and a time
  //           column line up down the page, so the eye reads columns not rows.
  //   dense — the split pane's result list, where 404px leaves room for the
  //           title, one line of context and the match figure and nothing else.
  let {
    item,
    onOpen,
    variant = "band",
    lead = "auto",
    meta,
    selected = false,
    dismissable = true,
  }: {
    item: ScrapedItem;
    onOpen: (item: ScrapedItem) => void;
    variant?: "band" | "dense";
    /** The leading visual. "auto" picks by type: logo for roles, a date tile
     *  for events, the type glyph for anything else. */
    lead?: "auto" | "logo" | "date" | "type" | "none";
    /** Overrides the metadata column, which is otherwise the location or source. */
    meta?: string;
    /** Highlights the row the split-pane detail column is showing. */
    selected?: boolean;
    dismissable?: boolean;
  } = $props();

  let logoFailed = $state(false);

  const parts = $derived(splitTitle(item));
  const isRole = $derived(item.item_type === "Internship" || item.item_type === "Job");
  const logo = $derived(logoFor(item.url));
  const TypeIcon = $derived(iconForType(item.item_type));
  const match = $derived(feed.skills.length > 0 ? skillMatch(item) : null);
  const dense = $derived(variant === "dense");

  const shape = $derived(
    lead !== "auto" ? lead : isRole ? "logo" : item.item_type === "Event" ? "date" : "type",
  );

  const when = $derived(new Date(item.timestamp));
  const month = $derived(when.toLocaleString(undefined, { month: "short" }));
  const day = $derived(String(when.getDate()).padStart(2, "0"));

  /** The fixed column: where it is, or failing that where it came from. */
  const metaText = $derived(
    meta ?? (isRole ? (item.location?.split("|")[0].trim() ?? "") : item.source_platform),
  );

  /**
   * The line under the title. A role names its company; anything else names its
   * source — unless the metadata column is already naming it, which is how an
   * article ended up printing "Phoronix" twice on the same row.
   */
  const subText = $derived(
    parts.secondary ?? (metaText === item.source_platform ? null : item.source_platform),
  );

  /** Everything the dense variant has room for, on one line. */
  const denseLine = $derived(
    [parts.secondary, isRole ? item.location?.split("|")[0].trim() : item.source_platform, timeAgo(item.timestamp)]
      .filter(Boolean)
      .join(" · "),
  );

  // A new item means a fresh logo attempt.
  $effect(() => {
    void item.url;
    logoFailed = false;
  });
</script>

<!--
  An article rather than a button: the row carries its own save and dismiss
  controls, which cannot nest inside another button. The title button is the
  accessible name; the stretched span makes the rest of the row clickable.
-->
<article
  aria-current={selected ? "true" : undefined}
  class="group relative flex items-center gap-3 border-t border-t-line-soft transition-colors first:border-t-transparent sm:gap-3.5
         {dense
    ? 'border-l-[3px] py-2.5 pr-3 pl-2'
    : 'rounded-xl px-1.5 py-2 sm:py-2.5'}
         {selected ? 'border-l-brand bg-hero' : 'border-l-transparent hover:bg-line-soft'}
         {item.seen && !selected ? 'opacity-70 hover:opacity-100' : ''}"
>
  {#if shape === "logo"}
    {#if logo && !logoFailed}
      <img
        src={logo}
        alt=""
        width="34"
        height="34"
        onerror={() => (logoFailed = true)}
        class="h-[34px] w-[34px] shrink-0 rounded-[10px] bg-line-soft object-contain p-1"
      />
    {:else}
      <div
        class="flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-[10px] bg-line-soft text-[13px] font-bold text-muted"
      >
        {(parts.secondary ?? parts.primary).charAt(0).toUpperCase()}
      </div>
    {/if}
  {:else if shape === "date"}
    <div class="w-[38px] shrink-0 rounded-[9px] bg-event-soft py-1 text-center">
      <div class="text-[9px] font-bold tracking-wider text-event uppercase">{month}</div>
      <div class="font-display text-[15px] leading-tight font-bold text-event">{day}</div>
    </div>
  {:else if shape === "type"}
    <div
      class="flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-[10px] {typeBadgeClass(
        item.item_type,
      )}"
    >
      <TypeIcon size={16} strokeWidth={2.5} />
    </div>
  {/if}

  <div class="min-w-0 flex-1">
    <h3 class="text-sm leading-snug font-semibold">
      <button
        onclick={() => onOpen(item)}
        class="line-clamp-2 text-left {selected ? 'text-brand-soft-fg' : 'hover:text-brand'}"
      >
        <span class="absolute inset-0" aria-hidden="true"></span>
        {parts.primary}
      </button>
    </h3>
    {#if dense || subText}
      <p class="truncate text-xs text-subtle {dense ? 'mt-0.5' : 'mt-px'}">
        {dense ? denseLine : subText}
      </p>
    {/if}
    <!-- Below sm the two fixed columns have nowhere to go, so they fold in. -->
    {#if !dense && metaText}
      <p class="truncate text-xs text-muted sm:hidden">{metaText} · {timeAgo(item.timestamp)}</p>
    {/if}
  </div>

  {#if !dense && metaText}
    <p class="hidden w-[150px] shrink-0 truncate text-xs text-muted sm:block lg:w-[220px]">
      {metaText}
    </p>
  {/if}

  {#if match}
    <span
      class="shrink-0 rounded-lg px-2 py-1 text-xs font-bold tabular-nums
             {match.have / match.total >= 0.6
        ? 'bg-brand-soft text-brand-soft-fg'
        : 'bg-line-soft text-subtle'}"
      title="You cover {match.have} of the {match.total} skills this posting asks for"
    >
      {match.have}/{match.total}
    </span>
  {/if}

  {#if !dense}
    <span class="hidden w-[50px] shrink-0 text-right text-xs text-subtle sm:block">
      {item.item_type === "Event" ? "" : timeAgo(item.timestamp)}
    </span>
  {/if}

  <!--
    Above the stretched link, or the row would swallow these clicks.

    Always rendered, never hover-revealed. These were `opacity-0
    group-hover:opacity-100`, which on Android means there is no way to save or
    dismiss anything from a list at all — hover does not exist on touch.
  -->
  <div class="relative z-10 flex shrink-0 items-center gap-0.5">
    <button
      onclick={() => feed.toggleSaved(item)}
      aria-pressed={item.saved}
      aria-label={item.saved ? "Remove from saved" : "Save this item"}
      title={item.saved ? "Remove from saved" : "Save"}
      class="tap h-9 w-9 sm:h-8 sm:w-8 {item.saved ? 'text-brand hover:text-brand' : ''}"
    >
      {#if item.saved}
        <BookmarkCheck size={16} strokeWidth={2.5} />
      {:else}
        <Bookmark size={16} />
      {/if}
    </button>
    {#if dismissable && !dense}
      <button
        onclick={() => feed.dismiss(item)}
        aria-label="Hide this item"
        title="Not interested"
        class="tap h-9 w-9 sm:h-8 sm:w-8"
      >
        <X size={16} />
      </button>
    {/if}
  </div>
</article>
