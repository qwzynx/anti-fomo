<script lang="ts">
  import { Check, CircleAlert, Plus, RotateCcw, Target } from "$lib/icons";
  import type { ResumeDoc, ResumeEntry, ResumeVariant, ResumeView } from "$lib/api";

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
   *
   * Sections and entries switch on and off too, and that is the coarse control
   * the fine one cannot stand in for: a graphics project is the reason to read
   * the résumé at one employer and six wasted lines at the next, and hiding it
   * a bullet at a time still leaves its heading and its dates on the page.
   * Only bullets have a third state — nothing else is ever trimmed to fit, so
   * for a section or an entry there is only "not this time".
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
  const onPageEntries = $derived(new Set(view.entries));
  const droppedIds = $derived(new Set(view.dropped.map((d) => d.id)));
  const coveredSet = $derived(new Set(view.covered));
  const missing = $derived(view.required.filter((s) => !coveredSet.has(s)));

  /** Sections holding something, so an empty one draws no heading here either. */
  const sections = $derived(doc.sections.filter((s) => s.entries.length > 0));

  /**
   * What an entry is called in this list.
   *
   * A skills section's "entries" are labelled lines rather than jobs — the
   * title is the label, the detail is the run — so they name themselves out of
   * different fields, and `org || title` covers both without asking which kind
   * of section this is.
   */
  function entryLabel(entry: ResumeEntry): string {
    return entry.org || entry.title || "Untitled";
  }

  /** The quieter second line: the role, and the dates that place it. */
  function entrySub(entry: ResumeEntry): string {
    const dates = [entry.start, entry.end]
      .map((d) => d.trim())
      .filter(Boolean)
      .join(" – ");
    const role = entry.org && entry.title ? entry.title : (entry.detail ?? "");
    return [role, dates].filter(Boolean).join(" · ");
  }

  function setInclude(id: string, state: "auto" | "in" | "out") {
    const include = { ...variant.include };
    if (state === "auto") delete include[id];
    else include[id] = state === "in";
    onVariant({ ...variant, include });
  }

  /**
   * On/off for a section or an entry.
   *
   * Two states rather than the bullets' three: neither is ever trimmed to fit,
   * so a pin would promise something the layout does not do.
   */
  function toggle(id: string) {
    setInclude(id, stateOf(id) === "out" ? "auto" : "out");
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
      <p class="text-xs font-semibold tracking-wide text-subtle uppercase">
        What goes on this résumé
      </p>
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

    <p class="mb-3 text-xs text-muted">
      Everything in your résumé, whether or not it made this page. Switch off what this
      employer has no use for and the trim gets those lines back for what they do.
    </p>

    {#each sections as section (section.id)}
      {@const sectionOff = stateOf(section.id) === "out"}
      <div class="mb-4">
        <div class="mb-1.5 flex items-center justify-between gap-2">
          <p
            class="text-[11px] font-bold tracking-wide uppercase {sectionOff
              ? 'text-subtle line-through'
              : 'text-subtle'}"
          >
            {section.title}
          </p>
          <button
            onclick={() => toggle(section.id)}
            aria-pressed={sectionOff}
            title={sectionOff ? "Put this section back" : "Leave this whole section off"}
            class="chip shrink-0 px-2 py-0.5 text-[10px] {sectionOff
              ? 'bg-danger text-danger-fg'
              : 'border border-line text-subtle hover:border-brand'}"
          >
            {sectionOff ? "Section off" : "Hide section"}
          </button>
        </div>

        {#each section.entries as entry (entry.id)}
          {@const entryOff = sectionOff || stateOf(entry.id) === "out"}
          {@const sub = entrySub(entry)}
          <div class="mt-2">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <p
                  class="text-xs font-semibold {entryOff
                    ? 'text-subtle line-through decoration-1'
                    : ''}"
                >
                  {entryLabel(entry)}
                </p>
                {#if sub}
                  <p class="text-[11px] leading-snug text-subtle">{sub}</p>
                {/if}
              </div>
              <button
                onclick={() => toggle(entry.id)}
                aria-pressed={stateOf(entry.id) === "out"}
                disabled={sectionOff}
                title={entryOff ? "Put this back on the page" : "Leave this off for this posting"}
                class="chip shrink-0 px-2 py-0.5 text-[10px] disabled:opacity-40 {stateOf(
                  entry.id,
                ) === 'out'
                  ? 'bg-danger text-danger-fg'
                  : 'border border-line text-subtle hover:border-brand'}"
              >
                {stateOf(entry.id) === "out" ? "Off" : "Hide"}
              </button>
            </div>

            {#if entry.bullets.length > 0}
              <ul class="mt-1 space-y-1 {entryOff ? 'opacity-50' : ''}">
                {#each entry.bullets as bullet (bullet.id)}
                  {@const state = stateOf(bullet.id)}
                  {@const shown = !entryOff && onPage.has(bullet.id)}
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
                          disabled={entryOff}
                          title="Always keep this bullet"
                          class="chip px-2 py-0.5 text-[10px] disabled:opacity-40 {state === 'in'
                            ? 'bg-brand text-brand-fg'
                            : 'border border-line text-subtle hover:border-brand'}"
                        >
                          Pin
                        </button>
                        <button
                          onclick={() => setInclude(bullet.id, state === "out" ? "auto" : "out")}
                          aria-pressed={state === "out"}
                          disabled={entryOff}
                          title="Leave this bullet off"
                          class="chip px-2 py-0.5 text-[10px] disabled:opacity-40 {state === 'out'
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
            {:else if !entryOff && !onPageEntries.has(entry.id)}
              <p class="mt-1 text-[11px] text-subtle">Not on the page.</p>
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </section>
</div>
