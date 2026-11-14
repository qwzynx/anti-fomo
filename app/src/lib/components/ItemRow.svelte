<script lang="ts">
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, BookmarkCheck, X, iconForType } from "$lib/icons";
  import { closesLabel, closesSoon, eventDate, monthDay, splitTitle, typeBadgeClass } from "$lib/item";
  import { highlight } from "$lib/query";

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
    marks = [],
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
    /** Search terms to highlight in the title. */
    marks?: string[];
  } = $props();

  let logoFailed = $state(false);

  const parts = $derived(splitTitle(item));
  /** Only split into runs when there is something to mark: most rows have nothing. */
  const titleRuns = $derived(marks.length > 0 ? highlight(parts.primary, marks) : null);
  const isRole = $derived(item.item_type === "Internship" || item.item_type === "Job");
  const logo = $derived(logoFor(item.url));
  const TypeIcon = $derived(iconForType(item.item_type));
  const match = $derived(feed.skills.length > 0 ? feed.match(item) : null);
  const saved = $derived(feed.isSaved(item.url));
  const seen = $derived(feed.isSeen(item.url));
  const dense = $derived(variant === "dense");

  const shape = $derived(
    lead !== "auto" ? lead : isRole ? "logo" : item.item_type === "Event" ? "date" : "type",
  );

  // Only for the date tile, and through one shared formatter. `Intl` builds a
  // fresh formatter per `toLocaleString` call with options, which is one of the
  // slowest things a row can do — and every row was paying it whether or not it
  // rendered a date.
  const date = $derived(shape === "date" ? monthDay(eventDate(item)) : null);

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

  /** A role's own deadline, when its page published one. */
  const deadline = $derived(isRole ? closesLabel(item) : null);
  // A last call is flagged by Rust a week out, so it is called out in the urgent colour even
  // before the three-day mark `closesSoon` uses.
  const urgent = $derived(isRole && (closesSoon(item) || item.last_call === true));

  /**
   * Everything the dense variant has room for, on one line: company/source and
   * location lead in the row's usual colour, then a trailing segment that gets
   * the urgency colour when it is a deadline rather than a plain timestamp.
   */
  const denseLead = $derived(
    [parts.secondary, isRole ? item.location?.split("|")[0].trim() : item.source_platform]
      .filter(Boolean)
      .join(" · "),
  );
  const denseTail = $derived(meta ?? deadline ?? timeAgo(item.timestamp));

  // A new item means a fresh logo attempt — and every list keys its `{#each}`
  // on `item.url`, so a new item *is* a new component and `logoFailed` starts
  // false on its own. The `$effect` that used to reset it here bought nothing
  // and cost one effect per row, of which there can be hundreds on screen.
</script>

<!--
  An article rather than a button: the row carries its own save and dismiss
  controls, which cannot nest inside another button. The title button is the
  accessible name; the stretched span makes the rest of the row clickable.
-->
<article
  aria-current={selected ? "true" : undefined}
  class="group relative flex items-center gap-3 border-b border-line-soft transition-colors last:border-b-0
         {dense ? 'py-2.5 pr-2 pl-4' : 'px-3 py-2.5 sm:px-4'}
         {selected ? 'bg-brand-soft/70' : 'hover:bg-hover'}"
>
  {#if selected}
    <span class="absolute inset-y-0 left-0 w-[2px] bg-brand" aria-hidden="true"></span>
  {/if}
  {#if shape === "logo"}
    {#if logo && !logoFailed}
      <img
        src={logo}
        alt=""
        width="30"
        height="30"
        loading="lazy"
        decoding="async"
        onerror={() => (logoFailed = true)}
        class="h-[30px] w-[30px] shrink-0 rounded-md border border-line-soft bg-white object-contain p-[3px]"
      />
    {:else}
      <div
        class="flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-md bg-line-soft text-[12px] font-semibold text-muted"
      >
        {(parts.secondary ?? parts.primary).charAt(0).toUpperCase()}
      </div>
    {/if}
  {:else if shape === "date" && date}
    <div class="w-[30px] shrink-0 rounded-md bg-event-soft py-0.5 text-center">
      <div class="text-[8.5px] font-bold tracking-wider text-event uppercase">{date.month}</div>
      <div class="text-[13px] leading-tight font-semibold text-event">{date.day}</div>
    </div>
  {:else if shape === "type"}
    <div
      class="flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-md {typeBadgeClass(
        item.item_type,
      )}"
    >
      <TypeIcon size={14} strokeWidth={2.25} />
    </div>
  {/if}

  <div class="min-w-0 flex-1">
    <h3 class="text-[13px] leading-snug {seen && !selected ? 'font-normal text-muted' : 'font-medium'}">
      <button
        onclick={() => onOpen(item)}
        class="line-clamp-2 text-left {selected ? 'text-foreground' : ''}"
      >
        <span class="absolute inset-0" aria-hidden="true"></span>
        {#if titleRuns}{#each titleRuns as run, i (i)}{#if run.hit}<mark
                class="rounded-sm bg-star-soft px-px text-inherit">{run.text}</mark
              >{:else}{run.text}{/if}{/each}{:else}{parts.primary}{/if}
      </button>
    </h3>
    {#if dense || subText}
      <p class="truncate text-[12px] text-subtle {dense ? 'mt-0.5' : 'mt-px'}">
        {#if dense}
          {denseLead}{denseLead ? " · " : ""}<span
            class={deadline && urgent ? "font-semibold text-danger" : ""}>{denseTail}</span
          >
        {:else}
          {subText}
        {/if}
      </p>
    {/if}
    <!-- Below sm the two fixed columns have nowhere to go, so they fold in. -->
    {#if !dense && metaText}
      <p class="truncate text-[12px] text-subtle sm:hidden">
        {metaText}{#if item.item_type !== "Event"}{" · "}{timeAgo(item.timestamp)}{/if}{#if deadline}{" · "}<span
            class={urgent ? "font-semibold text-danger" : ""}>{deadline}</span
          >{/if}
      </p>
    {/if}
  </div>

  {#if !dense && metaText}
    <div class="hidden w-[150px] shrink-0 sm:block lg:w-[200px]">
      <p class="truncate text-[12px] text-muted">{metaText}</p>
      {#if deadline}
        <p class="mt-0.5 truncate text-[11px] font-semibold {urgent ? 'text-danger' : 'text-subtle'}">
          {deadline}
        </p>
      {/if}
    </div>
  {/if}

  {#if match}
    <span
      class="shrink-0 rounded px-1.5 py-0.5 text-[11px] font-semibold tabular-nums
             {match.have / match.total >= 0.6
        ? 'bg-brand-soft text-brand-soft-fg'
        : 'bg-line-soft text-subtle'}"
      title="You cover {match.have} of the {match.total} skills this posting asks for"
    >
      {match.have}/{match.total}
    </span>
  {/if}

  {#if !dense}
    <span class="hidden w-[50px] shrink-0 text-right text-[12px] text-subtle tabular-nums sm:block">
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
      aria-pressed={saved}
      aria-label={saved ? "Remove from saved" : "Save this item"}
      title={saved ? "Remove from saved" : "Save"}
      class="tap h-8 w-8 sm:h-7 sm:w-7 {saved ? 'text-brand hover:text-brand' : ''}"
    >
      {#if saved}
        <BookmarkCheck size={15} strokeWidth={2.5} />
      {:else}
        <Bookmark size={15} />
      {/if}
    </button>
    {#if dismissable}
      <button
        onclick={() => feed.dismiss(item)}
        aria-label="Hide this item"
        title="Not interested"
        class="tap h-8 w-8 sm:h-7 sm:w-7 {dense ? 'reveal-on-hover' : ''}"
      >
        <X size={15} />
      </button>
    {/if}
  </div>
</article>
