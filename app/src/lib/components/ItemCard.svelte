<script lang="ts">
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, BookmarkCheck, Sparkles, Star, X, iconForType } from "$lib/icons";
  import { splitTitle, tagsFor, typeBadgeClass } from "$lib/item";
  import { skillMatch } from "$lib/skills";

  let {
    item,
    onOpen,
    index = 0,
    dismissable = true,
  }: {
    item: ScrapedItem;
    onOpen: (item: ScrapedItem) => void;
    index?: number;
    /** Off on the saved list, where hiding a starred item makes no sense. */
    dismissable?: boolean;
  } = $props();

  let logoFailed = $state(false);

  const logo = $derived(logoFor(item.url));
  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
  const TypeIcon = $derived(iconForType(item.item_type));
  const isTopMatch = $derived((item.relevance_score ?? 0) >= 15);
  // Only once the user has a profile: "0/8 skills" on every card is a worse
  // first impression than no badge at all.
  const match = $derived(feed.skills.length > 0 ? skillMatch(item) : null);
</script>

<!--
  An article rather than a <button>: the card holds its own save and dismiss
  buttons, and nesting those inside a button is invalid. The title button is the
  accessible name for opening the item; the surrounding click target is a
  convenience for pointer users.
-->
<article
  style="animation-delay: {Math.min(index, 12) * 30}ms"
  class="animate-fade-up group card relative flex flex-col gap-3 p-5 transition-all
         hover:-translate-y-0.5 hover:border-brand hover:shadow-lg hover:shadow-brand/5
         {item.seen ? 'opacity-65 hover:opacity-100' : ''}"
>
  <div class="flex items-start justify-between gap-2">
    <div class="flex items-center gap-2.5">
      {#if logo && !logoFailed}
        <img
          src={logo}
          alt=""
          width="36"
          height="36"
          onerror={() => (logoFailed = true)}
          class="h-9 w-9 rounded-lg bg-line-soft object-contain p-1"
        />
      {:else}
        <div
          class="flex h-9 w-9 items-center justify-center rounded-lg bg-line-soft text-sm font-bold text-muted"
        >
          {parts.primary.charAt(0).toUpperCase()}
        </div>
      {/if}
      <span
        class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase {typeBadgeClass(
          item.item_type,
        )}"
      >
        <TypeIcon size={11} strokeWidth={2.5} />
        {item.item_type}
      </span>
    </div>

    <!-- Above the stretched link below, or the card would swallow these clicks. -->
    <div class="relative z-10 flex shrink-0 items-center gap-1">
      {#if isTopMatch}
        <span
          class="inline-flex items-center gap-1 rounded-full bg-star-soft px-2 py-0.5 text-[10px] font-bold text-star"
          title="Matches your field and interests"
        >
          <Star size={10} strokeWidth={3} />
          TOP
        </span>
      {/if}
      <!-- Kept visible once saved so the state reads at a glance, otherwise
           revealed on hover to keep the card quiet. -->
      <button
        onclick={() => feed.toggleSaved(item)}
        aria-pressed={item.saved}
        aria-label={item.saved ? "Remove from saved" : "Save this item"}
        title={item.saved ? "Remove from saved" : "Save"}
        class="flex h-7 w-7 items-center justify-center rounded-lg text-subtle transition-all
               hover:bg-line-soft hover:text-brand focus-visible:opacity-100
               {item.saved ? 'text-brand opacity-100' : 'opacity-0 group-hover:opacity-100'}"
      >
        {#if item.saved}
          <BookmarkCheck size={16} strokeWidth={2.5} />
        {:else}
          <Bookmark size={16} />
        {/if}
      </button>
      {#if dismissable}
        <button
          onclick={() => feed.dismiss(item)}
          aria-label="Hide this item"
          title="Not interested"
          class="flex h-7 w-7 items-center justify-center rounded-lg text-subtle opacity-0 transition-all
                 hover:bg-line-soft hover:text-foreground group-hover:opacity-100 focus-visible:opacity-100"
        >
          <X size={16} />
        </button>
      {/if}
    </div>
  </div>

  <div class="flex flex-col gap-0.5">
    <h3 class="text-lg leading-tight font-bold">
      <button onclick={() => onOpen(item)} class="line-clamp-2 text-left hover:text-brand">
        <!-- Stretched link: makes the whole card clickable without nesting
             the action buttons inside another button. -->
        <span class="absolute inset-0 rounded-2xl" aria-hidden="true"></span>
        {parts.primary}
      </button>
    </h3>
    {#if parts.secondary}
      <p class="line-clamp-1 text-sm font-medium text-muted">{parts.secondary}</p>
    {/if}
    <span class="mt-0.5 text-xs font-medium text-subtle">{item.source_platform}</span>
  </div>

  {#if !parts.secondary}
    <p class="line-clamp-2 text-sm leading-relaxed text-muted">
      {item.content_text || "Open for details."}
    </p>
  {/if}

  {#if tags.length > 0}
    <div class="flex flex-wrap gap-1.5">
      {#each tags as tag (tag)}
        <span class="rounded-md bg-line-soft px-2 py-0.5 text-[11px] font-medium text-muted">
          {tag}
        </span>
      {/each}
    </div>
  {/if}

  <!-- Why this ranked where it did, in the user's own vocabulary. -->
  {#if (item.matched_interests && item.matched_interests.length > 0) || match}
    <div class="flex flex-wrap items-center gap-1.5 text-brand">
      <Sparkles size={12} />
      {#each item.matched_interests ?? [] as interest (interest)}
        <span class="rounded-md bg-brand-soft px-2 py-0.5 text-[11px] font-semibold text-brand-soft-fg">
          {interest}
        </span>
      {/each}
      {#if match}
        <span class="rounded-md bg-brand-soft px-2 py-0.5 text-[11px] font-bold text-brand-soft-fg">
          {match.have}/{match.total} skills
        </span>
      {/if}
    </div>
  {/if}

  <div
    class="mt-auto flex items-center justify-between border-t border-line-soft pt-3 text-[11px] font-medium text-subtle"
  >
    <span>{timeAgo(item.timestamp)}</span>
    <span class="text-brand opacity-0 transition-opacity group-hover:opacity-100">
      View details →
    </span>
  </div>
</article>
