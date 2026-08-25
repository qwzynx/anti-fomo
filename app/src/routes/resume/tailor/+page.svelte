<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import ResumePreview from "$lib/components/resume/ResumePreview.svelte";
  import TailorPanel from "$lib/components/resume/TailorPanel.svelte";
  import ThemePicker from "$lib/components/resume/ThemePicker.svelte";
  import { ArrowLeft, Download, FileText, Loader, Palette, SlidersHorizontal } from "$lib/icons";
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
   * Everything the tailoring decided comes back from a single `layout_resume`
   * call, not from a layout call plus a separate "what did you pick" call. The
   * two are not independent: the fit loop *is* the tailoring, so asking for
   * them apart would let the panel disagree with the page beside it about
   * which bullets survived.
   */

  const url = $derived(page.url.searchParams.get("url") ?? "");

  let view = $state.raw<ResumeView | null>(null);
  let variant = $state.raw<ResumeVariant | null>(null);
  let item = $state.raw<api.ScrapedItem | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let notice = $state<string | null>(null);
  let error = $state<string | null>(null);
  let tab = $state<"tailor" | "design">("tailor");
  let token = 0;

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
      if (!resume.exists) await resume.load();
      const [stored, posting] = await Promise.all([
        resume.id ? api.getResumeVariant(url, resume.id) : Promise.resolve(null),
        api.getItemDetail(url),
      ]);
      variant = stored ?? emptyVariant();
      item = posting;
      await relayout();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function relayout() {
    if (!resume.id) return;
    const mine = ++token;
    const next = await api.layoutResume({ id: resume.id, url, theme: variant?.theme ?? undefined });
    if (mine === token) view = next;
  }

  /** Saves the override and lays out again, so the page reflects the toggle. */
  async function applyVariant(next: ResumeVariant) {
    if (!resume.id) return;
    variant = next;
    await api.saveResumeVariant(url, resume.id, next);
    await relayout();
  }

  async function reset() {
    if (!resume.id) return;
    await api.clearResumeVariant(url, resume.id);
    variant = emptyVariant();
    await relayout();
  }

  async function download() {
    if (!view) return;
    busy = true;
    const outcome = await saveResumePdf({
      id: resume.id ?? undefined,
      url,
      theme: variant?.theme ?? undefined,
      filename: view.filename,
      reveal: true,
    });
    notice = outcomeMessage(outcome);
    busy = false;
  }

  $effect(() => {
    void url;
    void load();
  });

  const company = $derived(item?.company ?? null);
  const heading = $derived(item?.title ?? "this posting");
</script>

<svelte:head><title>Tailored résumé · Anti-FOMO</title></svelte:head>

<main class="mx-auto w-full max-w-7xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <div class="pt-4 pb-3">
    <button
      onclick={() => history.back()}
      class="mb-2 inline-flex items-center gap-1 text-sm font-semibold text-muted hover:text-foreground"
    >
      <ArrowLeft size={15} />
      Back
    </button>
    <h1 class="text-2xl leading-tight font-bold">Tailored résumé</h1>
    <p class="mt-0.5 text-sm text-muted">
      For <span class="font-semibold text-foreground">{heading}</span>{#if company}
        at {company}{/if}. Your résumé is untouched — this is a copy of it for this posting.
    </p>
  </div>

  {#if error}
    <p class="mb-3 rounded-xl bg-danger-soft p-3 text-sm text-danger" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="mb-3 rounded-xl bg-line-soft p-3 text-sm text-muted" role="status">{notice}</p>
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
    <div class="mt-10 flex justify-center"><Loader size={22} class="animate-spin text-subtle" /></div>
  {:else if !resume.exists}
    <EmptyState
      icon={FileText}
      title="No résumé yet"
      body="Build one first and every posting can tailor a copy of it."
      actionLabel="Build my résumé"
      onAction={() => goto("/resume")}
    />
  {:else if view && variant && resume.doc}
    <div class="lg:grid lg:grid-cols-[minmax(0,24rem)_minmax(0,1fr)] lg:gap-6">
      <div class="min-w-0 lg:order-2">
        <div class="mb-2 flex items-baseline justify-between gap-2">
          <p class="text-xs font-semibold tracking-wide text-subtle uppercase">Preview</p>
          <button
            onclick={download}
            disabled={busy}
            class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-3.5 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
          >
            {#if busy}<Loader size={15} class="animate-spin" />{:else}<Download size={15} />{/if}
            Save PDF
          </button>
        </div>
        <div class="rounded-xl bg-line-soft p-3 lg:sticky lg:top-[70px]">
          <ResumePreview pages={view.pages} scale={0.78} label="Résumé tailored to {heading}" />
        </div>
      </div>

      <div class="mt-6 min-w-0 lg:order-1 lg:mt-0">
        <div class="mb-3 flex flex-wrap gap-2">
          {#each [{ id: "tailor", label: "Tailoring", icon: SlidersHorizontal }, { id: "design", label: "Design", icon: Palette }] as const as t (t.id)}
            {@const Icon = t.icon}
            <button
              onclick={() => (tab = t.id)}
              aria-pressed={tab === t.id}
              class="chip inline-flex items-center gap-1.5 px-3.5 py-2 text-sm {tab === t.id
                ? 'bg-brand text-brand-fg'
                : 'border border-line text-muted hover:border-brand'}"
            >
              <Icon size={14} />
              {t.label}
            </button>
          {/each}
        </div>

        {#if tab === "design"}
          <section class="card p-5">
            <ThemePicker
              theme={view.theme}
              options={resume.themes}
              onChange={(next) => void applyVariant({ ...(variant ?? emptyVariant()), theme: next })}
            />
            <p class="mt-4 text-xs text-subtle">
              A version picked here applies to this posting only. The Design tab on your résumé sets
              the default for every other one.
            </p>
          </section>
        {:else}
          <TailorPanel
            doc={resume.doc}
            {view}
            {variant}
            onVariant={(next) => void applyVariant(next)}
            onReset={() => void reset()}
          />
        {/if}
      </div>
    </div>
  {/if}
</main>
