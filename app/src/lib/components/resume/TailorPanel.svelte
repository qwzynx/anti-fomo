<script lang="ts">
  import { Check, CircleAlert, Plus, RotateCcw, Target } from "$lib/icons";
  import type { ResumeDoc, ResumeVariant, ResumeView } from "$lib/api";

  /**
   * What the tailoring pass decided for this posting, and every way to overrule
   * it.
   *
   * The panel's job is to make the trim *visible*. Rust starts from the whole
   * résumé and takes bullets away until it fits the page budget, which is the
   * right default — the user wrote all of them on purpose — but a bullet
   * silently vanishing from a document reads as data loss. So everything that
   * came off the page is listed with a way to put it back, and everything that
   * stayed says which of the posting's skills it earned its line with.
   *
   * Three states per bullet: auto (Rust decides), pinned in, forced out. Pinned
   * bullets survive the page-budget trim entirely — that is what pinning means
   * — so the panel is the escape hatch for "this one matters, cut something
   * else".
   */
  let {
    doc,
    view,
    variant,
    onVariant,
    onReset,
  }: {
    doc: ResumeDoc;
    view: ResumeView;
    variant: ResumeVariant;
    onVariant: (next: ResumeVariant) => void;
    onReset: () => void;
  } = $props();

  const onPage = $derived(new Set(view.bullets));
  const droppedIds = $derived(new Set(view.dropped.map((d) => d.id)));
  const coveredSet = $derived(new Set(view.covered));
  const missing = $derived(view.required.filter((s) => !coveredSet.has(s)));

  /** Sections that actually have entries, so an empty one draws no heading. */
  const sections = $derived(doc.sections.filter((s) => s.entries.some((e) => e.bullets.length > 0)));

  function setInclude(id: string, state: "auto" | "in" | "out") {
    const include = { ...variant.include };
    if (state === "auto") delete include[id];
    else include[id] = state === "in";
    onVariant({ ...variant, include });
  }

  function stateOf(id: string): "auto" | "in" | "out" {
    const forced = variant.include[id];
    if (forced === undefined) return "auto";
    return forced ? "in" : "out";
  }

  const hasOverrides = $derived(
    Object.keys(variant.include).length > 0 ||
      Object.keys(variant.order).length > 0 ||
      Object.keys(variant.text).length > 0 ||
      variant.headline !== null,
  );
</script>

<div class="space-y-5">
  <!-- Coverage, stated the way the detail pane already states it. -->
  <section class="rounded-xl bg-line-soft p-4">
    <p class="mb-1.5 flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase">
      <Target size={12} />
      What this posting asks for
    </p>
    {#if view.required.length === 0}
      <p class="text-sm text-muted">
        This posting's page could not be read, so there are no skills to tailor against. The résumé
        below is your full one.
      </p>
    {:else}
      <p class="mb-2.5 text-sm">
        <span class="font-bold">{view.covered.length} of {view.required.length}</span>
        <span class="text-muted"> named on the page below</span>
      </p>
      <div class="flex flex-wrap gap-1.5">
        {#each view.required as skill (skill)}
          {@const covered = coveredSet.has(skill)}
          <span
            class="chip inline-flex items-center gap-1 px-2.5 py-1 {covered
              ? 'bg-job-soft text-job'
              : 'border border-line text-subtle'}"
          >
            {#if covered}<Check size={11} strokeWidth={3} />{/if}
            {skill}
          </span>
        {/each}
      </div>
      {#if missing.length > 0}
        <p class="mt-2.5 text-xs text-muted">
          Nothing in your résumé names {missing.length === 1 ? "the last one" : "the greyed ones"}.
          Nothing is invented here — if you have done that work, add a bullet that says so.
        </p>
      {/if}
    {/if}
  </section>

  {#if view.dropped.length > 0}
    <section class="rounded-xl border border-star/30 bg-star-soft/40 p-4">
      <p class="mb-1.5 flex items-center gap-1.5 text-xs font-bold tracking-wide uppercase text-star">
        <CircleAlert size={12} />
        Trimmed to fit
      </p>
      <p class="text-sm text-muted">
        {view.dropped.length}
        {view.dropped.length === 1 ? "bullet" : "bullets"} did not fit the page limit. They are still
        in your résumé — pin one below to keep it and something else will give way instead.
      </p>
    </section>
  {/if}

  <section>
    <div class="mb-2 flex items-baseline justify-between gap-3">
      <p class="text-xs font-semibold tracking-wide text-subtle uppercase">Bullets on this résumé</p>
      {#if hasOverrides}
        <button
          onclick={onReset}
          class="inline-flex shrink-0 items-center gap-1 text-xs font-semibold text-brand hover:underline"
        >
          <RotateCcw size={12} />
          Reset to auto
        </button>
      {/if}
    </div>

    {#each sections as section (section.id)}
      <div class="mb-4">
        <p class="mb-1.5 text-[11px] font-bold tracking-wide text-subtle uppercase">
          {section.title}
        </p>
        {#each section.entries as entry (entry.id)}
          {#if entry.bullets.length > 0}
            <p class="mt-2 mb-1 text-xs font-semibold">{entry.org || entry.title || "Untitled"}</p>
            <ul class="space-y-1">
              {#each entry.bullets as bullet (bullet.id)}
                {@const state = stateOf(bullet.id)}
                {@const shown = onPage.has(bullet.id)}
                {@const why = view.why[bullet.id] ?? []}
                <li
                  class="rounded-lg border p-2 {shown
                    ? 'border-line-soft bg-surface'
                    : 'border-dashed border-line bg-transparent'}"
                >
                  <div class="flex items-start gap-2">
                    <p
                      class="min-w-0 flex-1 text-[13px] leading-snug {shown
                        ? ''
                        : 'text-subtle line-through decoration-1'}"
                    >
                      {variant.text[bullet.id] ?? bullet.text}
                    </p>
                    <div class="flex shrink-0 gap-1">
                      <!-- Pin and hide are toggles back to auto, so there is
                           always a way back to what Rust would have chosen. -->
                      <button
                        onclick={() => setInclude(bullet.id, state === "in" ? "auto" : "in")}
                        aria-pressed={state === "in"}
                        title="Always keep this bullet"
                        class="chip px-2 py-0.5 text-[10px] {state === 'in'
                          ? 'bg-brand text-brand-fg'
                          : 'border border-line text-subtle hover:border-brand'}"
                      >
                        Pin
                      </button>
                      <button
                        onclick={() => setInclude(bullet.id, state === "out" ? "auto" : "out")}
                        aria-pressed={state === "out"}
                        title="Leave this bullet off"
                        class="chip px-2 py-0.5 text-[10px] {state === 'out'
                          ? 'bg-danger text-danger-fg'
                          : 'border border-line text-subtle hover:border-brand'}"
                      >
                        Hide
                      </button>
                    </div>
                  </div>

                  {#if why.length > 0}
                    <div class="mt-1.5 flex flex-wrap gap-1">
                      {#each why as skill (skill)}
                        <span
                          class="rounded-md bg-job-soft px-1.5 py-0.5 text-[10px] font-semibold text-job"
                        >
                          {skill}
                        </span>
                      {/each}
                    </div>
                  {:else if droppedIds.has(bullet.id)}
                    <p class="mt-1 flex items-center gap-1 text-[11px] text-subtle">
                      <Plus size={10} strokeWidth={3} class="rotate-45" />
                      Cut for space — names nothing this posting asked for
                    </p>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        {/each}
      </div>
    {/each}
  </section>
</div>
