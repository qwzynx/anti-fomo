<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { logoFor, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { ArrowUpRight, Bookmark, BookmarkCheck, MapPin, Sparkles, X, iconForType } from "$lib/icons";
  import { ctaLabel, sourceBlurb, splitTitle, tagsFor, typeBadgeClass } from "$lib/item";
  import JobSections from "./JobSections.svelte";
  import SkillMatch from "./SkillMatch.svelte";

  // Everything there is to say about one item, with no opinion about where it
  // is mounted. The modal wraps this in dialog chrome on phones; the jobs page
  // pins it in the right-hand column on desktop. Both need identical content,
  // and keeping one copy is what stops them drifting apart.
  let {
    item,
    onClose,
    titleId,
  }: {
    item: ScrapedItem;
    /** Renders a close button when the host is dismissable. */
    onClose?: () => void;
    titleId?: string;
  } = $props();

  let logoFailed = $state(false);

  const logo = $derived(logoFor(item.url));
  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
  const cta = $derived(ctaLabel(item));
  const TypeIcon = $derived(iconForType(item.item_type));
  /** Whether the enrichment pass reached this posting. */
  const enriched = $derived(
    Boolean(item.requirements || item.responsibilities || item.perks || item.description),
  );
  const locations = $derived(
    item.location
      ? item.location
          .split("|")
          .map((l) => l.trim())
          .filter(Boolean)
      : [],
  );
  // Discipline, attribute tags and source, deduped.
  const chips = $derived([
    ...new Set([item.discipline, ...tags, item.source_platform].filter(Boolean)),
  ]);

  // A new item means a fresh logo attempt.
  $effect(() => {
    void item.url;
    logoFailed = false;
  });

  // Opening an item counts as reading it, which sinks it in later rankings.
  $effect(() => {
    void feed.markSeen(item);
  });

  /** Links must leave the webview, not navigate the app out of itself. */
  async function open(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      console.error("could not open link", e);
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="flex items-start justify-between gap-4 border-b border-line-soft p-5 sm:p-6">
    <div class="flex min-w-0 items-center gap-3">
      {#if logo && !logoFailed}
        <img
          src={logo}
          alt=""
          width="48"
          height="48"
          onerror={() => (logoFailed = true)}
          class="h-12 w-12 shrink-0 rounded-xl bg-line-soft object-contain p-1.5"
        />
      {/if}
      <div class="min-w-0">
        <span
          class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase {typeBadgeClass(
            item.item_type,
          )}"
        >
          <TypeIcon size={11} strokeWidth={2.5} />
          {item.item_type}
        </span>
        <h2 id={titleId} class="mt-1.5 text-xl leading-tight font-bold">{parts.primary}</h2>
        {#if parts.secondary}
          <p class="text-sm font-medium text-muted">{parts.secondary}</p>
        {/if}
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-1">
      <button
        onclick={() => feed.toggleSaved(item)}
        aria-pressed={item.saved}
        aria-label={item.saved ? "Remove from saved" : "Save this item"}
        class="flex h-8 w-8 items-center justify-center rounded-full transition-colors hover:bg-line-soft
               {item.saved ? 'text-brand' : 'text-subtle hover:text-foreground'}"
      >
        {#if item.saved}
          <BookmarkCheck size={18} strokeWidth={2.5} />
        {:else}
          <Bookmark size={18} />
        {/if}
      </button>
      {#if onClose}
        <button
          onclick={onClose}
          aria-label="Close"
          class="flex h-8 w-8 items-center justify-center rounded-full text-subtle transition-colors hover:bg-line-soft hover:text-foreground"
        >
          <X size={18} />
        </button>
      {/if}
    </div>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto p-5 sm:p-6">
    <p class="mb-4 text-xs font-medium tracking-wide text-subtle uppercase">
      {sourceBlurb(item)} · {new Date(item.timestamp).toLocaleString()}
    </p>

    {#if item.matched_interests && item.matched_interests.length > 0}
      <div class="mb-4 flex flex-wrap items-center gap-1.5">
        <Sparkles size={14} class="text-brand" />
        <span class="text-xs font-semibold text-muted">Matches your interests:</span>
        {#each item.matched_interests as interest (interest)}
          <span
            class="rounded-md bg-brand-soft px-2 py-0.5 text-[11px] font-semibold text-brand-soft-fg"
          >
            {interest}
          </span>
        {/each}
      </div>
    {/if}

    <!-- Rust only fills `required_skills` for jobs and internships, so this is
         self-gating: an article never renders it. -->
    {#if item.required_skills && item.required_skills.length > 0}
      <SkillMatch {item} />
    {/if}

    {#if locations.length > 0}
      <div class="mb-4 rounded-xl bg-line-soft p-4">
        <p
          class="mb-1 flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase"
        >
          <MapPin size={12} />
          Locations
        </p>
        <p class="text-sm text-foreground">{locations.join(" · ")}</p>
        {#if item.location_tags && item.location_tags.length > 0}
          <div class="mt-2 flex flex-wrap gap-1.5">
            {#each item.location_tags as tag (tag)}
              <span
                class="rounded-md bg-surface px-2 py-0.5 text-[11px] font-semibold text-muted shadow-sm"
              >
                {tag}
              </span>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <JobSections {item} />

    <!-- The scraped one-liner is redundant once the real posting is here. -->
    {#if !enriched}
      <p class="text-[15px] leading-relaxed whitespace-pre-line text-muted">
        {item.content_text ||
          "No further details were scraped for this item — open the source for the full picture."}
      </p>
    {/if}

    {#if chips.length > 0}
      <div class="mt-6 flex flex-wrap gap-2">
        {#each chips as chip (chip)}
          <span class="rounded-full bg-line-soft px-3 py-1 text-xs font-medium text-muted">
            {chip}
          </span>
        {/each}
      </div>
    {/if}
  </div>

  <div class="shrink-0 border-t border-line-soft p-4">
    <button
      onclick={() => open(item.url)}
      class="flex w-full items-center justify-center gap-1.5 rounded-xl bg-brand py-3 text-center font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
    >
      {cta}
      <ArrowUpRight size={17} strokeWidth={2.5} />
    </button>
  </div>
</div>
