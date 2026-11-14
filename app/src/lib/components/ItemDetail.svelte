<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import {
    ArrowUpRight,
    Banknote,
    Bookmark,
    BookmarkCheck,
    EyeOff,
    FileText,
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
  import ApplicationTracker from "./ApplicationTracker.svelte";
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
    dismissable = true,
    onDismissed,
  }: {
    item: ScrapedItem;
    /** Renders a close button when the host is dismissable. */
    onClose?: () => void;
    titleId?: string;
    /** Offers "hide from feed". Off where hiding means nothing, e.g. Saved. */
    dismissable?: boolean;
    /** Told after a hide, so a modal host can close itself. */
    onDismissed?: (item: ScrapedItem) => void;
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
  /** The fetch itself failed — distinct from "fetched, and there was nothing". */
  let fetchFailed = $state(false);

  $effect(() => {
    const url = item.url;
    // Read the cache into a local first: assigning straight to `fetched` and
    // then checking `fetched !== undefined` below made this effect both read
    // and write the same state in one pass, which Svelte's dev build flags as
    // an infinite update loop — the write reschedules the effect, whose
    // rerun reads the value it just wrote, forever. `peeked` breaks the cycle.
    const peeked = feed.peekDetail(url);
    fetched = peeked;
    fetchFailed = false;
    if (peeked !== undefined) return;

    let live = true;
    feed.detail(url).then(
      (d) => {
        if (live) fetched = d;
      },
      () => {
        // Settle on the list row rather than spinning forever.
        if (!live) return;
        fetched = null;
        fetchFailed = true;
      },
    );
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
  /** Only a role has requirements to tailor a résumé against. */
  const isOpportunity = $derived(item.item_type === "Job" || item.item_type === "Internship");
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
  <!-- The action bar: what this is on the left, what you can do on the right. -->
  <div class="flex h-[44px] shrink-0 items-center gap-1 border-b border-line px-3">
    <span
      class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10.5px] font-semibold tracking-wide uppercase {typeBadgeClass(
        item.item_type,
      )}"
    >
      <TypeIcon size={11} strokeWidth={2.5} />
      {item.item_type}
    </span>
    <span class="ml-1.5 min-w-0 truncate text-[12px] text-subtle">via {item.source_platform}</span>
    <div class="ml-auto flex shrink-0 items-center gap-0.5">
      {#if dismissable}
        <button
          onclick={() => {
            void feed.dismiss(item);
            onDismissed?.(item);
          }}
          aria-label="Hide this from your feed"
          title="Hide (x)"
          class="tool"
        >
          <EyeOff size={15} />
        </button>
      {/if}
      <button
        onclick={() => feed.toggleSaved(item)}
        aria-pressed={saved}
        aria-label={saved ? "Remove from saved" : "Save this item"}
        title={saved ? "Saved (s)" : "Save (s)"}
        class="tool {saved ? 'text-brand hover:text-brand' : ''}"
      >
        {#if saved}
          <BookmarkCheck size={15} strokeWidth={2.5} />
        {:else}
          <Bookmark size={15} />
        {/if}
      </button>
      {#if onClose}
        <button onclick={onClose} aria-label="Close" class="tool">
          <X size={16} />
        </button>
      {/if}
    </div>
  </div>

  <div class="pane selectable flex-1 px-5 py-5 sm:px-6">
    <div class="mb-4 flex items-start gap-3">
      {#if logo && !logoFailed}
        <img
          src={logo}
          alt=""
          width="40"
          height="40"
          onerror={() => (logoFailed = true)}
          class="h-10 w-10 shrink-0 rounded-lg border border-line-soft bg-white object-contain p-1"
        />
      {/if}
      <div class="min-w-0">
        <h2 id={titleId} class="text-[19px] leading-snug font-semibold">{parts.primary}</h2>
        {#if parts.secondary}
          <p class="mt-0.5 text-[13px] font-medium text-muted">{parts.secondary}</p>
        {/if}
      </div>
    </div>

    <!-- The facts, as a property list: the way an app shows a record, not
         three coloured boxes. -->
    <dl class="mb-5 grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-[13px]">
      {#if locations.length > 0}
        <dt class="text-subtle">Location</dt>
        <dd class="min-w-0">
          {locations.join(" · ")}
          {#if item.location_tags && item.location_tags.length > 0}
            <span class="ml-1 text-subtle">({item.location_tags.join(", ")})</span>
          {/if}
        </dd>
      {/if}
      <dt class="text-subtle">Posted</dt>
      <dd>{timeAgo(item.timestamp)}</dd>
      {#if deadline}
        <dt class="text-subtle">Deadline</dt>
        <dd class="flex items-center gap-1.5 font-medium {urgent ? 'text-danger' : ''}">
          <Hourglass size={12} class="shrink-0" />{deadline}
        </dd>
      {/if}
      {#if pay}
        <dt class="text-subtle">Pay</dt>
        <dd class="flex items-center gap-1.5 font-medium"><Banknote size={12} class="shrink-0 text-subtle" />{pay}</dd>
      {/if}
      {#if item.seniority}
        <dt class="text-subtle">Level</dt>
        <dd>{item.seniority}</dd>
      {/if}
      <dt class="text-subtle">Source</dt>
      <dd class="text-muted">{sourceBlurb(item)}</dd>
    </dl>

    {#if isOpportunity}
      <ApplicationTracker {item} />
    {/if}

    {#if item.matched_interests && item.matched_interests.length > 0}
      <div class="mb-4 flex flex-wrap items-center gap-1.5">
        <Sparkles size={13} class="text-brand" />
        <span class="text-[12px] font-medium text-muted">Matches your interests</span>
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

    <JobSections item={full} />

    {#if pending}
      <div class="flex flex-col gap-2" aria-busy="true" aria-label="Loading the full posting">
        <div class="h-3 w-11/12 animate-pulse rounded bg-line-soft"></div>
        <div class="h-3 w-4/5 animate-pulse rounded bg-line-soft"></div>
        <div class="h-3 w-2/3 animate-pulse rounded bg-line-soft"></div>
      </div>
    {:else if fetchFailed}
      <p class="mb-3 text-xs text-danger" role="alert">
        The full posting could not be loaded from the local store. What the list knows is shown
        below.
      </p>
    {/if}

    <!-- The scraped one-liner is redundant once the real posting is here.
         Held back while the fetch is in flight: the note below says the page
         could not be read, and saying that before we have looked is a lie. -->
    {#if !enriched && !pending}
      <p class="text-[14px] leading-relaxed whitespace-pre-line text-muted">
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
          <span class="rounded bg-line-soft px-2 py-0.5 text-[12px] font-medium text-muted">
            {chip}
          </span>
        {/each}
      </div>
    {/if}
  </div>

  <div class="shrink-0 border-t border-line px-4 py-3">
    {#if openFailed}
      <p class="mb-2 text-center text-xs font-medium text-muted" role="alert">
        Could not hand this link to your browser. Open it directly: {item.url}
      </p>
    {/if}
    <div class="flex gap-2">
      <button
        onclick={() => open(item.url)}
        class="btn-primary h-9 flex-1"
      >
        {cta}
        <ArrowUpRight size={15} strokeWidth={2.5} />
      </button>
      <!-- Only where there is something to tailor against. Self-gating on
           `required_skills` the same way the skills panel above does, so an
           article never offers to write you a résumé. -->
      {#if isOpportunity}
        <a
          href="/resume/tailor?url={encodeURIComponent(item.url)}"
          onclick={onClose}
          class="tool tool-outline h-9 px-3 font-semibold text-foreground"
        >
          <FileText size={14} strokeWidth={2.2} />
          <span class="hidden sm:inline">Tailor résumé</span>
          <span class="sm:hidden">Résumé</span>
        </a>
      {/if}
    </div>
  </div>
</div>
