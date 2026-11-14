<script lang="ts">
  import { page } from "$app/state";
  import { afterNavigate, goto } from "$app/navigation";
  import { toast } from "$lib/toast.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import ResumePreview from "$lib/components/resume/ResumePreview.svelte";
  import TailorPanel from "$lib/components/resume/TailorPanel.svelte";
  import ThemePicker from "$lib/components/resume/ThemePicker.svelte";
  import {
    ArrowLeft,
    Download,
    Eye,
    FileText,
    Loader,
    Palette,
    RotateCcw,
    SlidersHorizontal,
  } from "$lib/icons";
  import * as api from "$lib/api";
  import type { ResumeVariant, ResumeView } from "$lib/api";
  import { outcomeMessage, saveResumePdf } from "$lib/resumeFile";
  import { resume } from "$lib/resume.svelte";

  /**
   * One posting, one résumé, tailored.
   *
   * The posting arrives as `?url=`, which is the whole state this route needs —
   * it makes the screen linkable and survives a reload, and it is why the
   * builder and this share one store rather than one passing the other a
   * document.
   *
   * Laid out like the rest of the app: a toolbar, then two panes that scroll
   * on their own — the outline of what goes on the page, and the page. Below
   * `lg:` there is room for one, so a switch picks which.
   *
   * Everything the tailoring decided comes back from a single `layout_resume`
   * call, not from a layout call plus a separate "what did you pick" call. The
   * two are not independent: the fit loop *is* the tailoring, so asking for
   * them apart would let the outline disagree with the page about which
   * bullets survived.
   */

  const url = $derived(page.url.searchParams.get("url") ?? "");

  let view = $state.raw<ResumeView | null>(null);
  let variant = $state.raw<ResumeVariant | null>(null);
  let item = $state.raw<api.ScrapedItem | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let tab = $state<"content" | "design">("content");
  /** Which pane a narrow window shows. */
  let narrowPane = $state<"edit" | "preview">("edit");

  /**
   * Whether this screen was reached from inside the app. Opened any other way
   * — a reload, a deep link — there is no history entry to go back to, and
   * `history.back()` did nothing at all.
   */
  let cameFromApp = $state(false);
  afterNavigate(({ from }) => {
    cameFromApp = from !== null;
  });

  function back() {
    if (cameFromApp) history.back();
    else void goto("/resume");
  }

  const emptyVariant = (): ResumeVariant => ({
    theme: null,
    headline: null,
    include: {},
    order: {},
    text: {},
    skills_lead: [],
  });

  async function load() {
    if (!url) {
      loading = false;
      return;
    }
    loading = true;
    error = null;
    try {
      // The résumé store is shared with the builder, so this screen edits the
      // same document the builder does — no second copy to fall out of step.
      await resume.ensure();
      const [stored, posting] = await Promise.all([
        resume.id ? api.getResumeVariant(url, resume.id) : Promise.resolve(null),
        api.getItemDetail(url),
      ]);
      variant = stored ?? emptyVariant();
      saved = variant;
      item = posting;
      await relayout();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  let layoutToken = 0;
  async function relayout() {
    if (!resume.id) return;
    const mine = ++layoutToken;
    try {
      const next = await api.layoutResume({ id: resume.id, url, theme: variant?.theme ?? undefined });
      if (mine === layoutToken) view = next;
    } catch (e) {
      if (mine === layoutToken) toast.error("lay out the tailored résumé", e);
    }
  }

  // --- saving overrides ---
  //
  // Ticking boxes is fast, and each tick is a save. Saves run one at a time
  // with the latest waiting, so two in flight cannot land out of order, and a
  // failure rolls back to the last copy that is actually on disk rather than
  // to whatever happened to be on screen when that click began.

  /** The last variant the database confirmed. */
  let saved: ResumeVariant | null = null;
  let queued: ResumeVariant | null = null;
  let flushing = false;

  async function applyVariant(next: ResumeVariant) {
    if (!resume.id || !url) return;
    variant = next;
    queued = next;
    if (flushing) return;
    flushing = true;
    // Pinned for the whole run: a jump to another posting mid-queue must not
    // write this one's choices under that one's URL.
    const id = resume.id;
    const target = url;
    try {
      while (queued) {
        const v = queued;
        queued = null;
        try {
          await api.saveResumeVariant(target, id, v);
          saved = v;
        } catch (e) {
          queued = null;
          variant = saved ?? emptyVariant();
          toast.error("save that change", e);
          break;
        }
      }
    } finally {
      flushing = false;
    }
    await relayout();
  }

  const hasOverrides = $derived(
    !!variant &&
      (Object.keys(variant.include).length > 0 ||
        Object.keys(variant.order).length > 0 ||
        Object.keys(variant.text).length > 0 ||
        variant.headline !== null ||
        variant.theme !== null),
  );

  async function reset() {
    if (!resume.id) return;
    const ok = await confirm({
      title: "Reset this tailoring?",
      body: "Pins, items you left off, rewordings, the headline and the design chosen for this posting all go back to automatic. Your résumé itself is not touched.",
      confirmLabel: "Reset",
      danger: true,
    });
    if (!ok) return;
    queued = null;
    try {
      await api.clearResumeVariant(url, resume.id);
    } catch (e) {
      toast.error("reset the tailoring", e);
      return;
    }
    variant = emptyVariant();
    saved = variant;
    await relayout();
  }

  async function download() {
    if (!view) return;
    busy = true;
    try {
      const outcome = await saveResumePdf({
        id: resume.id ?? undefined,
        url,
        theme: variant?.theme ?? undefined,
        filename: view.filename,
        reveal: true,
      });
      const message = outcomeMessage(outcome);
      if (message) outcome.ok ? toast.success(message) : toast.error("save the PDF", message);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    void url;
    void load();
  });

  const heading = $derived(item?.title ?? "this posting");
  const subtitle = $derived(
    item
      ? `${item.title}${item.company ? ` · ${item.company}` : ""} — from “${resume.name}”`
      : `From “${resume.name}”`,
  );
</script>

<svelte:head><title>Tailored résumé · Anti-FOMO</title></svelte:head>

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title="Tailor résumé" {subtitle}>
    {#snippet actions()}
      <button onclick={back} class="tool" title="Back"><ArrowLeft size={14} /><span class="hidden sm:inline">Back</span></button>
      {#if hasOverrides}
        <button onclick={reset} class="tool" title="Undo every choice made for this posting">
          <RotateCcw size={14} /><span class="hidden sm:inline">Reset</span>
        </button>
      {/if}
      {#if view}
        <button onclick={download} disabled={busy} class="btn-primary ml-1 disabled:opacity-50">
          {#if busy}<Loader size={14} class="animate-spin" />{:else}<Download size={14} />{/if}
          Save PDF
        </button>
      {/if}
    {/snippet}
  </PageHeader>

  {#if error}
    <p class="shrink-0 border-b border-danger/30 bg-danger-soft px-4 py-2 text-[12.5px] text-danger" role="alert">
      {error}
    </p>
  {/if}

  {#if !url}
    <EmptyState
      icon={FileText}
      title="No posting"
      body="Open a role and use “Tailor résumé” to get here."
      actionLabel="Browse roles"
      onAction={() => goto("/internships")}
    />
  {:else if loading}
    <div class="flex flex-1 items-center justify-center">
      <Loader size={22} class="animate-spin text-subtle" />
    </div>
  {:else if !resume.exists}
    <EmptyState
      icon={FileText}
      title="No résumé yet"
      body="Build one first and every posting can tailor a copy of it."
      actionLabel="Build my résumé"
      onAction={() => goto("/resume")}
    />
  {:else if view && variant && resume.doc}
    <!-- One pane at a time below lg:, chosen here. -->
    <div class="flex shrink-0 border-b border-line px-3 py-2 lg:hidden">
      <div class="flex w-full items-center rounded-md border border-line bg-surface p-0.5" role="group" aria-label="Show">
        {#each [{ id: "edit", label: "Edit", icon: SlidersHorizontal }, { id: "preview", label: `Preview · ${view.pages.length} ${view.pages.length === 1 ? "page" : "pages"}`, icon: Eye }] as const as p (p.id)}
          {@const Icon = p.icon}
          <button
            onclick={() => (narrowPane = p.id)}
            aria-pressed={narrowPane === p.id}
            class="inline-flex h-7 flex-1 items-center justify-center gap-1.5 rounded text-[12.5px] font-medium transition-colors {narrowPane ===
            p.id
              ? 'bg-hover text-foreground'
              : 'text-subtle hover:text-foreground'}"
          >
            <Icon size={13} />
            {p.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="flex min-h-0 flex-1">
      <!-- The outline -->
      <div
        class="min-h-0 w-full flex-col lg:flex lg:w-[440px] lg:shrink-0 lg:border-r lg:border-line xl:w-[480px] {narrowPane ===
        'edit'
          ? 'flex'
          : 'hidden'}"
      >
        <div class="flex shrink-0 items-center gap-1 border-b border-line px-3 pt-2">
          {#each [{ id: "content", label: "Content", icon: SlidersHorizontal }, { id: "design", label: "Design", icon: Palette }] as const as t (t.id)}
            {@const Icon = t.icon}
            <button
              onclick={() => (tab = t.id)}
              aria-pressed={tab === t.id}
              class="-mb-px inline-flex items-center gap-1.5 border-b-2 px-2.5 pb-2 text-[13px] font-medium transition-colors {tab ===
              t.id
                ? 'border-brand text-foreground'
                : 'border-transparent text-subtle hover:text-foreground'}"
            >
              <Icon size={13} />
              {t.label}
            </button>
          {/each}
        </div>

        {#if tab === "design"}
          <div class="pane flex-1 px-4 py-4">
            <ThemePicker
              theme={view.theme}
              options={resume.themes}
              onChange={(next) => void applyVariant({ ...(variant ?? emptyVariant()), theme: next })}
            />
            <p class="mt-4 text-[12px] text-subtle">
              A design picked here applies to this posting only. The Design tab on your résumé sets
              the default for every other one.
            </p>
          </div>
        {:else}
          {#key url}
            <TailorPanel
              doc={resume.doc}
              {view}
              {variant}
              onVariant={(next) => void applyVariant(next)}
            />
          {/key}
        {/if}
      </div>

      <!-- The page -->
      <div class="min-h-0 min-w-0 flex-1 flex-col lg:flex {narrowPane === 'preview' ? 'flex' : 'hidden'}">
        <ResumePreview
          pages={view.pages}
          maxPages={view.theme.max_pages}
          fill={view.fill}
          label="Résumé tailored to {heading}"
        />
      </div>
    </div>
  {/if}
</div>
