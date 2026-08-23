<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { timeAgo, type ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { ArrowUpRight, Bookmark, BookmarkCheck, Flame, MapPin, X } from "$lib/icons";
  import { ctaLabel, splitTitle, tagsFor } from "$lib/item";

  // The one decision worth making today, given the whole width of the window.
  //
  // Home used to open on three summary boxes, a setup banner, a search field,
  // four selects, a density switch and a chip row before the first result. This
  // is what replaced all of it: the highest-ranked role you have not seen,
  // stated once, with the two things you would actually do about it and the
  // figure that says whether it is worth doing.
  let {
    item,
    onOpen,
  }: {
    item: ScrapedItem;
    onOpen: (item: ScrapedItem) => void;
  } = $props();

  /** Set when the opener refuses or is unavailable, so the click is not silent. */
  let openFailed = $state(false);

  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
  const cta = $derived(ctaLabel(item));

  const required = $derived(item.required_skills ?? []);
  const split = $derived(feed.splitSkills(item));
  const have = $derived(split.have);
  const missing = $derived(split.missing);
  const saved = $derived(feed.isSaved(item.url));
  /** One segment per skill the posting asks for, so the bar is the count. */
  const segments = $derived(Math.min(required.length, 12));
  const filled = $derived(
    required.length === 0 ? 0 : Math.round((have.length / required.length) * segments),
  );
  const showMeter = $derived(required.length >= 2 && feed.skills.length > 0);

  $effect(() => {
    void item.url;
    openFailed = false;
  });

  /** Links leave the webview; they never navigate the app out of itself. */
  async function apply() {
    try {
      await openUrl(item.url);
      openFailed = false;
      void feed.markSeen(item);
    } catch (e) {
      console.error("could not open link", e);
      openFailed = true;
    }
  }
</script>

<div class="border-y border-hero-line bg-hero">
  <div
    class="mx-auto flex w-full max-w-6xl flex-col gap-5 px-4 py-5 sm:px-6 lg:flex-row lg:items-start lg:gap-7 lg:px-8"
  >
    <div class="min-w-0 flex-1">
      <p class="mb-2.5 flex items-center gap-1.5 text-[11px] font-bold tracking-[0.1em] text-star uppercase">
        <Flame size={14} strokeWidth={2.5} class="shrink-0" />
        Your best match today
      </p>

      <h2 class="font-display text-xl leading-tight font-bold tracking-tight text-pretty sm:text-[25px]">
        <button onclick={() => onOpen(item)} class="text-left hover:text-brand">
          {parts.primary}
        </button>
      </h2>
      {#if parts.secondary}
        <p class="mt-1 text-[15px] font-medium text-muted">{parts.secondary}</p>
      {/if}

      <div class="mt-3.5 flex flex-wrap gap-1.5">
        {#each tags as tag, i (tag)}
          <!-- `tagsFor` leads with the location when the posting has one, so
               the pin is earned by the first chip only in that case. -->
          {@const isPlace = i === 0 && Boolean(item.location)}
          <span
            class="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-semibold
                   {isPlace ? 'bg-job-soft text-job' : 'bg-surface text-muted'}"
          >
            {#if isPlace}<MapPin size={11} strokeWidth={2.5} />{/if}
            {tag}
          </span>
        {/each}
        <span class="rounded-full bg-surface px-2.5 py-1 text-xs font-semibold text-muted">
          via {item.source_platform}
        </span>
        <span class="rounded-full bg-surface px-2.5 py-1 text-xs font-semibold text-muted">
          {timeAgo(item.timestamp)}
        </span>
      </div>

      <!-- On a phone the meter belongs here, inline, rather than in a side
           panel there is no room for. -->
      {#if showMeter}
        <div class="mt-4 flex items-center gap-3 lg:hidden">
          <div class="flex flex-1 gap-[3px]" role="img" aria-label="You cover {have.length} of {required.length} skills">
            {#each { length: segments } as _, i (i)}
              <span
                class="h-1.5 flex-1 rounded-full {i < filled ? 'bg-brand' : 'bg-line'}"
              ></span>
            {/each}
          </div>
          <span class="shrink-0 text-[13px] font-bold text-brand tabular-nums">
            {have.length} of {required.length}
          </span>
        </div>
      {/if}

      {#if openFailed}
        <p class="mt-3 text-xs font-medium text-muted" role="alert">
          Could not hand this link to your browser. Open it directly: {item.url}
        </p>
      {/if}

      <div class="mt-4 flex items-center gap-2">
        <button
          onclick={apply}
          class="flex h-[46px] flex-1 items-center justify-center gap-2 rounded-xl bg-brand px-5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover sm:h-[42px] sm:flex-none"
        >
          {cta}
          <ArrowUpRight size={15} strokeWidth={2.5} class="shrink-0" />
        </button>

        <!-- 46px targets, always visible. The list controls this replaces were
             hover-only, which is to say absent on the phone build. -->
        <button
          onclick={() => feed.toggleSaved(item)}
          aria-pressed={saved}
          aria-label={saved ? "Remove from saved" : "Save this role"}
          class="flex h-[46px] w-[46px] shrink-0 items-center justify-center gap-2 rounded-xl border border-line bg-surface text-muted transition-colors hover:border-subtle sm:h-[42px] sm:w-auto sm:px-4 sm:text-sm sm:font-semibold
                 {saved ? 'text-brand' : ''}"
        >
          {#if saved}
            <BookmarkCheck size={16} strokeWidth={2.5} />
          {:else}
            <Bookmark size={16} />
          {/if}
          <span class="hidden sm:inline">{saved ? "Saved" : "Save"}</span>
        </button>

        <button
          onclick={() => feed.dismiss(item)}
          aria-label="Not for me — show the next match"
          class="flex h-[46px] w-[46px] shrink-0 items-center justify-center rounded-xl border border-line bg-surface text-subtle transition-colors hover:text-foreground sm:h-[42px] sm:w-auto sm:border-transparent sm:bg-transparent sm:px-3 sm:text-sm sm:font-medium"
        >
          <X size={16} class="sm:hidden" />
          <span class="hidden sm:inline">Not for me</span>
        </button>
      </div>
    </div>

    <!-- The skill panel gets more room here than it ever had on a card. -->
    {#if showMeter}
      <div class="hidden w-[248px] shrink-0 rounded-[15px] bg-surface p-4 lg:block">
        <div class="mb-2.5 flex items-baseline justify-between">
          <span class="text-[11px] font-bold tracking-wider text-subtle uppercase">You cover</span>
          <span class="font-display text-xl font-bold text-brand tabular-nums">
            {have.length}<span class="text-[13px] font-semibold text-subtle">/{required.length}</span>
          </span>
        </div>
        <div class="mb-3 flex gap-[3px]" role="img" aria-label="{have.length} of {required.length} skills">
          {#each { length: segments } as _, i (i)}
            <span class="h-[5px] flex-1 rounded-full {i < filled ? 'bg-brand' : 'bg-line'}"></span>
          {/each}
        </div>
        <div class="flex flex-wrap gap-1.5">
          {#each have.slice(0, 6) as skill (skill)}
            <span class="rounded-[7px] bg-brand-soft px-2 py-0.5 text-[11px] font-semibold text-brand-soft-fg">
              {skill}
            </span>
          {/each}
          {#each missing.slice(0, 4) as skill (skill)}
            <span class="rounded-[7px] border border-dashed border-line px-2 py-0.5 text-[11px] font-medium text-subtle">
              {skill}
            </span>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
