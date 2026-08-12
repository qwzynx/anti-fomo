<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { logoFor, type ScrapedItem } from "$lib/api";
  import { ctaLabel, sourceBlurb, splitTitle, tagsFor, typeBadgeClass } from "$lib/item";

  let { item, onClose }: { item: ScrapedItem | null; onClose: () => void } = $props();

  let logoFailed = $state(false);

  const logo = $derived(item ? logoFor(item.url) : null);
  const parts = $derived(item ? splitTitle(item) : { primary: "", secondary: null });
  const tags = $derived(item ? tagsFor(item) : []);
  const cta = $derived(item ? ctaLabel(item) : "");
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

  // The backdrop scroll-locks the page underneath while open.
  $effect(() => {
    if (!item) return;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = "";
    };
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

<svelte:window
  onkeydown={(e) => {
    if (item && e.key === "Escape") onClose();
  }}
/>

{#if item}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
  <div
    class="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
    onclick={onClose}
    role="dialog"
    aria-modal="true"
    aria-label={item.title}
    tabindex="-1"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      onclick={(e) => e.stopPropagation()}
      class="animate-fade-up safe-bottom flex max-h-[88vh] w-full max-w-xl flex-col overflow-hidden rounded-t-3xl border border-zinc-200 bg-white shadow-2xl sm:rounded-3xl dark:border-zinc-800 dark:bg-zinc-900"
    >
      <div
        class="flex items-start justify-between gap-4 border-b border-zinc-100 p-6 dark:border-zinc-800"
      >
        <div class="flex items-center gap-3">
          {#if logo && !logoFailed}
            <img
              src={logo}
              alt=""
              width="48"
              height="48"
              onerror={() => (logoFailed = true)}
              class="h-12 w-12 rounded-xl bg-zinc-100 object-contain p-1.5 dark:bg-zinc-800"
            />
          {/if}
          <div>
            <span
              class="rounded-full px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase {typeBadgeClass(
                item.item_type,
              )}"
            >
              {item.item_type}
            </span>
            <h2 class="mt-1.5 text-xl leading-tight font-bold">{parts.primary}</h2>
            {#if parts.secondary}
              <p class="text-sm font-medium text-zinc-600 dark:text-zinc-400">{parts.secondary}</p>
            {/if}
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <button
            onclick={() => open(item.url)}
            class="hidden rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-indigo-500 sm:block"
          >
            {cta}
          </button>
          <button
            onclick={onClose}
            aria-label="Close"
            class="flex h-8 w-8 items-center justify-center rounded-full text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
          >
            ✕
          </button>
        </div>
      </div>

      <div class="flex-1 overflow-y-auto p-6">
        <p class="mb-4 text-xs font-medium tracking-wide text-zinc-400 uppercase">
          {sourceBlurb(item)} · {new Date(item.timestamp).toLocaleString()}
        </p>

        {#if locations.length > 0}
          <div class="mb-4 rounded-xl bg-zinc-50 p-4 dark:bg-zinc-800/60">
            <p class="mb-1 text-xs font-bold tracking-wide text-zinc-400 uppercase">📍 Locations</p>
            <p class="text-sm text-zinc-700 dark:text-zinc-300">{locations.join(" · ")}</p>
            {#if item.location_tags && item.location_tags.length > 0}
              <div class="mt-2 flex flex-wrap gap-1.5">
                {#each item.location_tags as tag (tag)}
                  <span
                    class="rounded-md bg-white px-2 py-0.5 text-[11px] font-semibold text-zinc-600 shadow-sm dark:bg-zinc-700 dark:text-zinc-300"
                  >
                    {tag}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        {/if}

        <p class="text-[15px] leading-relaxed whitespace-pre-line text-zinc-700 dark:text-zinc-300">
          {item.content_text ||
            "No further details were scraped for this item — open the source for the full picture."}
        </p>

        {#if chips.length > 0}
          <div class="mt-6 flex flex-wrap gap-2">
            {#each chips as chip (chip)}
              <span
                class="rounded-full bg-zinc-100 px-3 py-1 text-xs font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400"
              >
                {chip}
              </span>
            {/each}
          </div>
        {/if}
      </div>

      <div class="border-t border-zinc-100 p-4 dark:border-zinc-800">
        <button
          onclick={() => open(item.url)}
          class="block w-full rounded-xl bg-indigo-600 py-3 text-center font-semibold text-white transition-all hover:bg-indigo-500 hover:shadow-lg hover:shadow-indigo-600/25"
        >
          {cta}
        </button>
      </div>
    </div>
  </div>
{/if}
