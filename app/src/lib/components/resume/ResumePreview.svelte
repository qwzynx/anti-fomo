<script lang="ts">
  import { tick } from "svelte";
  import type { ResumePage } from "$lib/api";
  import { ChevronLeft, ChevronRight, File, Files, ZoomIn, ZoomOut } from "$lib/icons";
  import { previewPrefs } from "$lib/previewPrefs.svelte";
  import ResumeSheet from "./ResumeSheet.svelte";

  /**
   * The résumé as a document viewer.
   *
   * Two ways to move through it, the reader's choice and remembered: one page
   * at a time with buttons (and ←/→), or every page in one column to scroll
   * through. Zoom is the same in both: − and + step through fixed levels,
   * Ctrl + wheel or Ctrl +/− do the same, and clicking the percentage goes
   * back to fitting the pane — the whole page when paging, the page's width
   * when scrolling. 100% is the paper's real size.
   *
   * It owns its box and its scrolling, so page two is reachable from the
   * viewer itself rather than by scrolling an editor beside it. It fills its
   * parent, so the parent must give it a height.
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

  const PAD = 24;
  const GAP = 20;
  /** CSS pixels per point at 100%: a point is 1/72 in, a CSS pixel 1/96. */
  const ACTUAL = 96 / 72;
  const STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

  const mode = $derived(previewPrefs.mode);

  let index = $state(0);
  /** In scroll mode, the page nearest the top of the view. */
  let scrolledTo = $state(0);
  const last = $derived(Math.max(0, pages.length - 1));
  // A relayout can take a page away; the viewer must not point past the end.
  const current = $derived(Math.min(mode === "scroll" ? scrolledTo : index, last));

  /** `null` fits the pane; otherwise a zoom where 1 is the paper's real size. */
  let zoom = $state<number | null>(null);

  let stage = $state<HTMLDivElement | null>(null);
  let stageW = $state(0);
  let stageH = $state(0);
  const sheets: HTMLElement[] = [];

  const first = $derived(pages[0]);
  const fitScale = $derived.by(() => {
    if (!first || stageW === 0) return 0;
    const byWidth = (stageW - PAD * 2) / first.width;
    const byHeight = (stageH - PAD * 2) / first.height;
    return Math.max(0.15, mode === "paged" ? Math.min(byWidth, byHeight) : byWidth);
  });
  /** CSS pixels per point. */
  const scale = $derived(zoom === null ? fitScale : zoom * ACTUAL);
  const percent = $derived(Math.round((scale / ACTUAL) * 100));

  /** Changes the zoom keeping the middle of the view where it was. */
  async function setZoom(next: number | null) {
    const el = stage;
    const fx = el && el.scrollWidth > 0 ? (el.scrollLeft + el.clientWidth / 2) / el.scrollWidth : 0.5;
    const fy = el && el.scrollHeight > 0 ? (el.scrollTop + el.clientHeight / 2) / el.scrollHeight : 0;
    zoom = next;
    await tick();
    if (!el) return;
    el.scrollLeft = fx * el.scrollWidth - el.clientWidth / 2;
    el.scrollTop = fy * el.scrollHeight - el.clientHeight / 2;
  }

  function zoomIn() {
    const now = scale / ACTUAL;
    void setZoom(STEPS.find((s) => s > now + 0.005) ?? STEPS[STEPS.length - 1]);
  }

  function zoomOut() {
    const now = scale / ACTUAL;
    void setZoom([...STEPS].reverse().find((s) => s < now - 0.005) ?? STEPS[0]);
  }

  // Ctrl + wheel zooms. Registered by hand because a wheel listener added
  // through the template is passive, and this one has to stop the scroll.
  $effect(() => {
    const el = stage;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      if (e.deltaY < 0) zoomIn();
      else if (e.deltaY > 0) zoomOut();
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  function go(n: number) {
    const next = Math.max(0, Math.min(last, n));
    if (mode === "scroll") {
      const sheet = sheets[next];
      if (sheet && stage) stage.scrollTo({ top: sheet.offsetTop - PAD, behavior: "smooth" });
      scrolledTo = next;
      return;
    }
    if (next === current) return;
    index = next;
    stage?.scrollTo({ top: 0 });
  }

  function onScroll() {
    if (mode !== "scroll" || !stage) return;
    // The last page whose top has passed the upper third of the view.
    const line = stage.scrollTop + stage.clientHeight / 3;
    let n = 0;
    for (let i = 0; i < pages.length; i++) if ((sheets[i]?.offsetTop ?? Infinity) <= line) n = i;
    scrolledTo = n;
  }

  function setMode(next: "paged" | "scroll") {
    if (next === mode) return;
    const at = current;
    previewPrefs.mode = next;
    index = at;
    scrolledTo = at;
    // Land on the page that was being read, in either mode.
    void tick().then(() => {
      if (next === "scroll") {
        const sheet = sheets[at];
        if (sheet && stage) stage.scrollTop = sheet.offsetTop - PAD;
      } else stage?.scrollTo({ top: 0 });
    });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || e.altKey) return;
    const el = e.target as HTMLElement | null;
    if (el?.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el?.tagName ?? "")) return;
    if (e.ctrlKey || e.metaKey) {
      if (e.key === "=" || e.key === "+") zoomIn();
      else if (e.key === "-") zoomOut();
      else if (e.key === "0") void setZoom(null);
      else return;
      e.preventDefault();
      return;
    }
    // Scrolling owns Page Up/Down and Home/End whenever there is a scroll —
    // scroll mode, or a page zoomed past the pane. The arrows always turn pages.
    const paging = mode === "paged" && zoom === null;
    const to =
      e.key === "ArrowLeft" || (paging && e.key === "PageUp")
        ? current - 1
        : e.key === "ArrowRight" || (paging && e.key === "PageDown")
          ? current + 1
          : paging && e.key === "Home"
            ? 0
            : paging && e.key === "End"
              ? last
              : null;
    if (to === null) return;
    e.preventDefault();
    go(to);
  }

  const shown = $derived(mode === "scroll" ? pages : pages.slice(current, current + 1));
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
      class="tool min-w-7 px-0 disabled:opacity-35"
    >
      <ChevronLeft size={15} />
    </button>
    <span class="min-w-[4.25rem] text-center text-[12px] text-muted tabular-nums" aria-live="polite">
      Page <span class="font-semibold text-foreground">{current + 1}</span> of {pages.length}
    </span>
    <button
      onclick={() => go(current + 1)}
      disabled={current >= last}
      aria-label="Next page"
      class="tool min-w-7 px-0 disabled:opacity-35"
    >
      <ChevronRight size={15} />
    </button>

    <span
      class="ml-1 hidden truncate rounded-md px-2 py-0.5 text-[11px] font-semibold @sm:inline {over
        ? 'bg-star-soft text-star'
        : 'bg-job-soft text-job'}"
      title={over ? "Over the page limit set under Design" : "Within the page limit set under Design"}
    >
      {limitLabel}
    </span>
    {#if fill !== null && !over}
      <span class="hidden truncate text-[11px] text-subtle @3xl:inline">
        · last page {Math.round(fill * 100)}% full
      </span>
    {/if}

    <div class="ml-auto flex shrink-0 items-center gap-1">
      <div class="flex items-center rounded-md border border-line bg-surface p-0.5" role="group" aria-label="Page view">
        {#each [{ id: "paged", icon: File, title: "One page at a time" }, { id: "scroll", icon: Files, title: "Scroll through every page" }] as const as m (m.id)}
          {@const Icon = m.icon}
          <button
            onclick={() => setMode(m.id)}
            aria-pressed={mode === m.id}
            aria-label={m.title}
            title={m.title}
            class="flex h-6 w-7 items-center justify-center rounded transition-colors {mode === m.id
              ? 'bg-hover text-foreground'
              : 'text-subtle hover:text-foreground'}"
          >
            <Icon size={13} />
          </button>
        {/each}
      </div>

      <div class="flex items-center rounded-md border border-line bg-surface p-0.5" role="group" aria-label="Zoom">
        <button
          onclick={zoomOut}
          disabled={zoom !== null && zoom <= STEPS[0]}
          aria-label="Zoom out"
          title="Zoom out (Ctrl −)"
          class="flex h-6 w-6 items-center justify-center rounded text-subtle hover:text-foreground disabled:opacity-35"
        >
          <ZoomOut size={13} />
        </button>
        <button
          onclick={() => void setZoom(null)}
          aria-pressed={zoom === null}
          title={zoom === null ? "Fitted to the pane" : "Fit to the pane (Ctrl 0)"}
          class="h-6 min-w-[3rem] rounded px-1 text-[11.5px] font-medium tabular-nums transition-colors {zoom === null
            ? 'text-subtle'
            : 'text-foreground hover:bg-hover'}"
        >
          {zoom === null ? `Fit · ${percent}%` : `${percent}%`}
        </button>
        <button
          onclick={zoomIn}
          disabled={zoom !== null && zoom >= STEPS[STEPS.length - 1]}
          aria-label="Zoom in"
          title="Zoom in (Ctrl +)"
          class="flex h-6 w-6 items-center justify-center rounded text-subtle hover:text-foreground disabled:opacity-35"
        >
          <ZoomIn size={13} />
        </button>
      </div>
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

    <!-- Focusable so the keyboard reaches it once it is clicked. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      bind:this={stage}
      bind:clientWidth={stageW}
      bind:clientHeight={stageH}
      onscroll={onScroll}
      tabindex="0"
      class="min-w-0 flex-1 overflow-auto overscroll-contain bg-line-soft [scrollbar-gutter:stable] focus-visible:outline-none"
    >
      {#if scale > 0}
        <!-- `w-max` with `min-w-full`, not a centring flex row alone: content
             wider than the pane must scroll to its left edge, which a centred
             overflow cannot. -->
        <div
          class="relative flex min-h-full w-max min-w-full flex-col items-center {mode === 'paged' &&
          zoom === null
            ? 'justify-center'
            : ''}"
          style="padding: {PAD}px; gap: {GAP}px"
        >
          {#each shown as p, i (mode === "scroll" ? i : current)}
            {@const n = mode === "scroll" ? i : current}
            <div bind:this={sheets[n]} class="flex flex-col items-center gap-1.5">
              <ResumeSheet page={p} width={p.width * scale} class="rounded-sm shadow-lg ring-1 ring-black/10" />
              {#if mode === "scroll" && pages.length > 1}
                <span class="text-[11px] text-subtle tabular-nums">Page {n + 1}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>
