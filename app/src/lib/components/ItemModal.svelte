<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { logoFor, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import {
    ArrowUpRight,
    Bookmark,
    BookmarkCheck,
    MapPin,
    Sparkles,
    X,
    iconForType,
  } from "$lib/icons";
  import { ctaLabel, sourceBlurb, splitTitle, tagsFor, typeBadgeClass } from "$lib/item";

  let { item, onClose }: { item: ScrapedItem | null; onClose: () => void } = $props();

  let logoFailed = $state(false);
  let panel = $state<HTMLElement | null>(null);
  let titleId = $derived(item ? `modal-title-${encodeURIComponent(item.url)}` : undefined);

  const logo = $derived(item ? logoFor(item.url) : null);
  const parts = $derived(item ? splitTitle(item) : { primary: "", secondary: null });
  const tags = $derived(item ? tagsFor(item) : []);
  const cta = $derived(item ? ctaLabel(item) : "");
  const TypeIcon = $derived(iconForType(item?.item_type ?? "Article"));
  const locations = $derived(
    item?.location
      ? item.location
          .split("|")
          .map((l) => l.trim())
          .filter(Boolean)
      : [],
  );
  // Discipline, attribute tags and source, deduped.
  const chips = $derived(
    item ? [...new Set([item.discipline, ...tags, item.source_platform].filter(Boolean))] : [],
  );

  // A new item means a fresh logo attempt.
  $effect(() => {
    void item?.url;
    logoFailed = false;
  });

  // Opening an item counts as reading it, which sinks it in later rankings.
  $effect(() => {
    if (item) void feed.markSeen(item);
  });

  // The backdrop scroll-locks the page underneath while open.
  $effect(() => {
    if (!item) return;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = "";
    };
  });

  // Move focus into the dialog on open and hand it back on close, so keyboard
  // and screen-reader users aren't dropped at the top of the document.
  $effect(() => {
    if (!item || !panel) return;
    const previous = document.activeElement as HTMLElement | null;
    panel.focus();
    return () => previous?.focus?.();
  });

  /** Everything focusable inside the panel, in document order. */
  function focusables(): HTMLElement[] {
    if (!panel) return [];
    return [
      ...panel.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => el.offsetParent !== null);
  }

  /** Keeps Tab cycling inside the dialog rather than escaping to the page. */
  function trapFocus(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const nodes = focusables();
    if (nodes.length === 0) return;

    const first = nodes[0];
    const last = nodes[nodes.length - 1];
    const active = document.activeElement;

    if (e.shiftKey && (active === first || active === panel)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  /** Links must leave the webview, not navigate the app out of itself. */
  async function open(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      console.error("could not open link", e);
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (!item) return;
    if (e.key === "Escape") onClose();
    else trapFocus(e);
  }}
/>

{#if item}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
    onclick={onClose}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      bind:this={panel}
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
      class="animate-fade-up safe-bottom card flex max-h-[88vh] w-full max-w-xl flex-col overflow-hidden rounded-t-3xl bg-elevated shadow-2xl focus:outline-none sm:rounded-3xl"
    >
      <div class="flex items-start justify-between gap-4 border-b border-line-soft p-6">
        <div class="flex items-center gap-3">
          {#if logo && !logoFailed}
            <img
              src={logo}
              alt=""
              width="48"
              height="48"
              onerror={() => (logoFailed = true)}
              class="h-12 w-12 rounded-xl bg-line-soft object-contain p-1.5"
            />
          {/if}
          <div>
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
          <button
            onclick={onClose}
            aria-label="Close"
            class="flex h-8 w-8 items-center justify-center rounded-full text-subtle transition-colors hover:bg-line-soft hover:text-foreground"
          >
            <X size={18} />
          </button>
        </div>
      </div>

      <div class="flex-1 overflow-y-auto p-6">
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

        <p class="text-[15px] leading-relaxed whitespace-pre-line text-muted">
          {item.content_text ||
            "No further details were scraped for this item — open the source for the full picture."}
        </p>

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

      <div class="border-t border-line-soft p-4">
        <button
          onclick={() => open(item.url)}
          class="flex w-full items-center justify-center gap-1.5 rounded-xl bg-brand py-3 text-center font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
        >
          {cta}
          <ArrowUpRight size={17} strokeWidth={2.5} />
        </button>
      </div>
    </div>
  </div>
{/if}
