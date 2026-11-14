<script lang="ts">
  import type { ResumePage } from "$lib/api";
  import { ChevronLeft, ChevronRight } from "$lib/icons";
  import ResumeSheet from "./ResumeSheet.svelte";

  /**
   * The résumé as a document viewer: one page at a time, scaled to the space
   * it has, with a page strip beside it when there is more than one.
   *
   * It used to draw every page in one column at a fixed scale, inside whatever
   * scrolled the rest of the screen — so page two of a tailored résumé was
   * only reachable by scrolling the editor past everything in it. A viewer
   * that owns its own box and its own scrolling is what lets the page beside
   * an editor be read as a page.
   *
   * It fills its parent, so the parent must give it a height.
   */
  let {
    pages,
    label = "Résumé preview",
    /** The theme's page limit, so going over it is said here, every time. */
    maxPages = 1,
    /** How full the last page is, 0–1, when the caller knows. */
    fill = null,
  }: {
    pages: ResumePage[];
    label?: string;
    maxPages?: number;
    fill?: number | null;
  } = $props();

  let index = $state(0);
  // A relayout can take a page away; the viewer must not point past the end.
  const current = $derived(Math.min(index, Math.max(0, pages.length - 1)));
  const page = $derived(pages[current]);

  type Fit = "page" | "width";
  let fit = $state<Fit>("page");

  let stage = $state<HTMLDivElement | null>(null);
  let stageW = $state(0);
  let stageH = $state(0);
  const PAD = 24;

  /** Whole page in view, or the page as wide as the pane and scrolled. */
  const sheetWidth = $derived.by(() => {
    if (!page || stageW === 0) return 0;
    const byWidth = stageW - PAD * 2;
    const byHeight = ((stageH - PAD * 2) * page.width) / page.height;
    const w = fit === "width" ? byWidth : Math.min(byWidth, byHeight);
    // Never so small it cannot be read, never blown up past 160%.
    return Math.max(160, Math.min(w, page.width * 1.6));
  });

  function go(n: number) {
    const next = Math.max(0, Math.min(pages.length - 1, n));
    if (next === current) return;
    index = next;
    stage?.scrollTo({ top: 0 });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
    const el = e.target as HTMLElement | null;
    if (el?.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el?.tagName ?? "")) return;
    // In Width mode the stage scrolls, so Page Up/Down and Home/End belong to
    // the scroll; only the arrows turn pages there.
    const paging = fit === "page";
    const to =
      e.key === "ArrowLeft" || (paging && e.key === "PageUp")
        ? current - 1
        : e.key === "ArrowRight" || (paging && e.key === "PageDown")
          ? current + 1
          : paging && e.key === "Home"
            ? 0
            : paging && e.key === "End"
              ? pages.length - 1
              : null;
    if (to === null) return;
    e.preventDefault();
    go(to);
  }

  const over = $derived(pages.length > maxPages);
  const limitLabel = $derived(
    over
      ? `${pages.length} pages · limit ${maxPages}`
      : `Fits ${maxPages} ${maxPages === 1 ? "page" : "pages"}`,
  );
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="@container flex h-full min-h-0 flex-col" role="region" aria-label={label} onkeydown={onKeydown}>
  <div class="flex h-10 shrink-0 items-center gap-1 border-b border-line px-2">
    <button
      onclick={() => go(current - 1)}
      disabled={current === 0}
      aria-label="Previous page"
      class="tool disabled:opacity-35"
    >
      <ChevronLeft size={15} />
    </button>
    <span class="min-w-[4.5rem] text-center text-[12px] text-muted tabular-nums" aria-live="polite">
      Page <span class="font-semibold text-foreground">{current + 1}</span> of {pages.length}
    </span>
    <button
      onclick={() => go(current + 1)}
      disabled={current >= pages.length - 1}
      aria-label="Next page"
      class="tool disabled:opacity-35"
    >
      <ChevronRight size={15} />
    </button>

    <span
      class="ml-1.5 truncate rounded-md px-2 py-0.5 text-[11px] font-semibold {over
        ? 'bg-star-soft text-star'
        : 'bg-job-soft text-job'}"
      title={over ? "Over the page limit set under Design" : "Within the page limit set under Design"}
    >
      {limitLabel}
    </span>
    {#if fill !== null && !over}
      <span class="hidden truncate text-[11px] text-subtle @2xl:inline">
        · last page {Math.round(fill * 100)}% full
      </span>
    {/if}

    <div
      class="ml-auto flex shrink-0 items-center rounded-md border border-line bg-surface p-0.5"
      role="group"
      aria-label="Zoom"
    >
      {#each [{ id: "page", label: "Page" }, { id: "width", label: "Width" }] as const as f (f.id)}
        <button
          onclick={() => (fit = f.id)}
          aria-pressed={fit === f.id}
          title={f.id === "page" ? "Fit the whole page" : "Fit the page's width"}
          class="h-6 rounded px-2 text-[11.5px] font-medium transition-colors {fit === f.id
            ? 'bg-hover text-foreground'
            : 'text-subtle hover:text-foreground'}"
        >
          {f.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="flex min-h-0 flex-1">
    {#if pages.length > 1}
      <nav
        class="pane hidden w-[92px] shrink-0 flex-col items-center gap-3 border-r border-line py-3 @xl:flex"
        aria-label="Pages"
      >
        {#each pages as p, n (n)}
          <button
            onclick={() => go(n)}
            aria-current={n === current ? "page" : undefined}
            aria-label="Page {n + 1}"
            class="group flex flex-col items-center gap-1"
          >
            <ResumeSheet
              page={p}
              width={62}
              class="rounded-[2px] ring-1 transition-shadow {n === current
                ? 'ring-2 ring-brand'
                : 'ring-black/15 group-hover:ring-brand/60'}"
            />
            <span
              class="text-[11px] tabular-nums {n === current
                ? 'font-semibold text-foreground'
                : 'text-subtle'}">{n + 1}</span
            >
          </button>
        {/each}
      </nav>
    {/if}

    <!-- Focusable so the arrow keys and Page Up/Down turn pages once it is clicked. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      bind:this={stage}
      bind:clientWidth={stageW}
      bind:clientHeight={stageH}
      tabindex="0"
      class="pane min-w-0 flex-1 bg-line-soft [scrollbar-gutter:stable] focus-visible:outline-none"
    >
      {#if page && sheetWidth > 0}
        <div
          class="flex min-h-full justify-center {fit === 'page' ? 'items-center' : 'items-start'}"
          style="padding: {PAD}px"
        >
          <ResumeSheet {page} width={sheetWidth} class="rounded-sm shadow-lg ring-1 ring-black/10" />
        </div>
      {/if}
    </div>
  </div>
</div>
