<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import {
    ArrowUpRight,
    Banknote,
    Bookmark,
    BookmarkCheck,
    Hourglass,
    MapPin,
    Sparkles,
    X,
    iconForType,
  } from "$lib/icons";
  import {
    closesLabel,
    closesSoon,
    ctaLabel,
    formatPay,
    hasScrapedPosting,
    sourceBlurb,
    splitTitle,
    tagsFor,
    typeBadgeClass,
  } from "$lib/item";
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
  /** Set when the opener refuses or is unavailable, so the click is not silent. */
  let openFailed = $state(false);

  /**
   * The posting in full.
   *
   * `item` is a list row, and a list row deliberately carries no description —
   * shipping all four fetched fields with every row is what made the hub a
   * 45.6 MB payload. This is the only place any of it is rendered, so this is
   * where it is asked for.
   */
  let fetched = $state<ScrapedItem | null | undefined>(undefined);

  $effect(() => {
    const url = item.url;
    fetched = feed.peekDetail(url);
    if (fetched !== undefined) return;

    let live = true;
    void feed.detail(url).then((d) => {
      if (live) fetched = d;
    });
    return () => {
      live = false;
    };
  });

  /** The list row until the fetch lands, so the header paints immediately. */
  const full = $derived(fetched && fetched.url === item.url ? fetched : item);
  const pending = $derived(fetched === undefined);

  const logo = $derived(logoFor(item.url));
  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
  const cta = $derived(ctaLabel(item));
  const TypeIcon = $derived(iconForType(item.item_type));
  const saved = $derived(feed.isSaved(item.url));
  /** Whether the enrichment pass reached this posting. */
  const enriched = $derived(hasScrapedPosting(full) || Boolean(full.perks));
  const locations = $derived(
    item.location
      ? item.location
          .split("|")
          .map((l) => l.trim())
          .filter(Boolean)
      : [],
  );
  /** Where it is, who carried it, and how old — the line under the title. */
  const meta = $derived(
    [locations[0], `via ${item.source_platform}`, timeAgo(item.timestamp)]
      .filter(Boolean)
      .join(" · "),
  );

  const deadline = $derived(closesLabel(item));
  const urgent = $derived(closesSoon(item));
  const pay = $derived(formatPay(item));

  // Discipline, attribute tags and source, deduped.
  const chips = $derived([
    ...new Set([item.discipline, ...tags, item.source_platform].filter(Boolean)),
  ]);

  // A new item means a fresh logo attempt, and a clean slate for the link.
  $effect(() => {
    void item.url;
    logoFailed = false;
    openFailed = false;
  });

  // Opening an item counts as reading it, which sinks it in later rankings.
  $effect(() => {
    void feed.markSeen(item);
  });

  /**
   * Links must leave the webview, not navigate the app out of itself.
   *
   * A refusal here is worth showing. The opener plugin checks the URL against
   * a scope before it will open anything, so a capability missing
   * `opener:allow-default-urls` turns every apply button into a no-op that
   * only says so in the console — which is exactly how it went unnoticed.
   */
  async function open(url: string) {
    try {
      await openUrl(url);
      openFailed = false;
    } catch (e) {
      console.error("could not open link", e);
      openFailed = true;
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
        aria-pressed={saved}
        aria-label={saved ? "Remove from saved" : "Save this item"}
        class="flex h-8 w-8 items-center justify-center rounded-full transition-colors hover:bg-line-soft
               {saved ? 'text-brand' : 'text-subtle hover:text-foreground'}"
      >
        {#if saved}
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
    <!-- Where and when, then what the source itself is. The two used to be one
         all-caps line that ran the width of the pane and shouted a full
         timestamp at a reader who wanted "3 hours ago". -->
    <p class="text-sm text-muted">{meta}</p>
    <p class="mb-4 text-xs text-subtle">{sourceBlurb(item)}</p>

    {#if deadline || pay}
      <div class="mb-4 flex flex-wrap gap-2">
        {#if deadline}
          <div class="min-w-[140px] flex-1 rounded-xl p-3 {urgent ? 'bg-danger-soft' : 'bg-line-soft'}">
            <p
              class="flex items-center gap-1.5 text-[11px] font-bold tracking-wide uppercase {urgent
                ? 'text-danger'
                : 'text-subtle'}"
            >
              <Hourglass size={12} />
              Deadline
            </p>
            <p class="mt-1 text-sm font-semibold {urgent ? 'text-danger' : 'text-foreground'}">
              {deadline}
            </p>
          </div>
        {/if}
        {#if pay}
          <div class="min-w-[140px] flex-1 rounded-xl bg-line-soft p-3">
            <p class="flex items-center gap-1.5 text-[11px] font-bold tracking-wide text-subtle uppercase">
              <Banknote size={12} />
              Compensation
            </p>
            <p class="mt-1 text-sm font-semibold text-foreground">{pay}</p>
          </div>
        {/if}
      </div>
    {/if}

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

    {#if locations.length > 1 || (item.location_tags?.length ?? 0) > 0}
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

    <JobSections item={full} />

    <!-- The scraped one-liner is redundant once the real posting is here.
         Held back while the fetch is in flight: the note below says the page
         could not be read, and saying that before we have looked is a lie. -->
    {#if !enriched && !pending}
      <p class="text-[15px] leading-relaxed whitespace-pre-line text-muted">
        {full.content_text ||
          "No further details were scraped for this item — open the source for the full picture."}
      </p>
      {#if item.item_type === "Internship" || item.item_type === "Job"}
        <!-- Says why the skills panel is missing. Nothing is inferred from the
             job title any more, so a posting we could not read shows no skills
             at all, and an unexplained gap reads as a bug. -->
        <p class="mt-3 text-xs text-subtle">
          This posting's own page could not be read, so there are no requirements or skills to
          match against. Open it on the source to see the full description.
        </p>
      {/if}
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
    {#if openFailed}
      <p class="mb-2 text-center text-xs font-medium text-muted" role="alert">
        Could not hand this link to your browser. Open it directly: {item.url}
      </p>
    {/if}
    <button
      onclick={() => open(item.url)}
      class="flex w-full items-center justify-center gap-1.5 rounded-xl bg-brand py-3 text-center font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
    >
      {cta}
      <ArrowUpRight size={17} strokeWidth={2.5} />
    </button>
  </div>
</div>
