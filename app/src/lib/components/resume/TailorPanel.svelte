<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import {
    Check,
    ChevronDown,
    ChevronsDownUp,
    ChevronsUpDown,
    CircleAlert,
    EyeOff,
    GripVertical,
    Save,
    ListChecks,
    Pencil,
    Pin,
    Target,
    Undo2,
    X,
  } from "$lib/icons";
  import { moved, sortHandle } from "$lib/sortable";
  import { detailItems, detailKey, skillItems, skillKey } from "$lib/resumeItems";
  import type {
    ResumeBullet,
    ResumeDoc,
    ResumeEntry,
    ResumeSection,
    ResumeVariant,
    ResumeView,
  } from "$lib/api";

  /**
   * What the tailoring pass decided for this posting, and every way to overrule
   * it — laid out as an outline of the résumé with one checkbox per line.
   *
   * **The checkbox means "on this page".** Ticked is on the page, unticked is
   * not, whoever decided it. A click always does what it looks like it does:
   * unticking leaves a bullet off, ticking one pins it — a pinned bullet is
   * never trimmed to fit, which is the only way to promise it will appear. A
   * click never hands a bullet back to the app's choice, because the app may
   * well choose what it chose before and the click would look dead; that is a
   * separate, quieter action on rows that carry an override.
   *
   * Rust starts from the whole résumé and takes bullets away until it fits the
   * page budget. A bullet silently vanishing reads as data loss, so every row
   * says why it is where it is: pinned, left off by you, cut to fit, and which
   * of the posting's skills it earned its line with.
   *
   * Sections and entries switch on and off too — a graphics project is the
   * reason to read the résumé at one employer and six wasted lines at the next,
   * and hiding it a bullet at a time would still leave its heading and dates.
   * Neither is ever trimmed, so for them there is only "not this time".
   *
   * The master résumé is never touched: every choice here is a per-posting
   * override the page saves through `onVariant`.
   */
  let {
    doc,
    view,
    variant,
    byHand = false,
    onVariant,
    onSaveToResume,
  }: {
    doc: ResumeDoc;
    view: ResumeView;
    variant: ResumeVariant;
    /** Made without a job: there is nothing to match, only what to keep. */
    byHand?: boolean;
    onVariant: (next: ResumeVariant) => void;
    /**
     * Makes a rewording permanent: writes it into the résumé itself, where
     * every version and every posting gets it, and drops this override.
     */
    onSaveToResume?: (bulletId: string, text: string) => void;
  } = $props();

  type Filter = "all" | "on" | "off";
  type Include = "auto" | "in" | "out";

  let filter = $state<Filter>("all");
  /** A skill from the posting: show only the bullets that name it. */
  let skill = $state<string | null>(null);
  let matchOpen = $state(false);
  /** Entries opened to show their bullets. Everything starts folded. */
  const expanded = new SvelteSet<string>();

  // --- what the posting asks for ---

  const onPage = $derived(new Set(view.bullets));
  const droppedIds = $derived(new Set(view.dropped.map((d) => d.id)));
  const coveredSet = $derived(new Set(view.covered));
  const missing = $derived(view.required.filter((s) => !coveredSet.has(s)));
  // What the posting called non-negotiable, drawn apart from what it merely
  // preferred: "covers 8 of 11" is the wrong number to lead with when the
  // three it does not cover are the three the posting insisted on.
  const mustSet = $derived(new Set(view.must));
  const split = $derived(mustSet.size > 0 && mustSet.size < view.required.length);
  const mustCovered = $derived(view.must.filter((s) => coveredSet.has(s)).length);
  const ordered = $derived(
    [...view.required].sort((a, b) => Number(mustSet.has(b)) - Number(mustSet.has(a))),
  );
  const coverage = $derived(
    view.required.length === 0 ? 0 : view.covered.length / view.required.length,
  );

  // --- overrides ---

  function stateOf(id: string): Include {
    const forced = variant.include[id];
    if (forced === undefined) return "auto";
    return forced ? "in" : "out";
  }

  function setInclude(id: string, state: Include) {
    const include = { ...variant.include };
    if (state === "auto") delete include[id];
    else include[id] = state === "in";
    onVariant({ ...variant, include });
  }

  /** Sections and entries: on, or not this time. */
  function toggleBlock(id: string) {
    setInclude(id, stateOf(id) === "out" ? "auto" : "out");
  }

  // --- the outline, with each bullet's state worked out once per pass ---

  interface Row {
    bullet: ResumeBullet;
    state: Include;
    /**
     * On the page. An override is answered at once rather than after the
     * relayout comes back, so a click shows its result before Rust has.
     */
    shown: boolean;
    why: string[];
    echo: string[];
  }

  /** One item of a detail line or a skills run, switchable on its own. */
  interface Item {
    name: string;
    key: string;
    on: boolean;
  }

  interface EntryModel {
    entry: ResumeEntry;
    /** The stack line's technologies, or a skills line's skills. */
    items: Item[];
    off: boolean;
    rows: Row[];
    onCount: number;
    visible: Row[];
  }

  interface SectionModel {
    section: ResumeSection;
    off: boolean;
    entries: EntryModel[];
    onCount: number;
    total: number;
  }

  /** The reader has taken over: nothing is trimmed or moved automatically. */
  const frozen = $derived(variant.frozen);
  const over = $derived(view.pages.length > view.theme.max_pages);

  const narrowing = $derived(filter !== "all" || skill !== null);

  function passes(row: Row): boolean {
    if (filter === "on" && !row.shown) return false;
    if (filter === "off" && row.shown) return false;
    if (skill !== null && !row.bullet.skills.includes(skill) && !row.why.includes(skill)) {
      return false;
    }
    return true;
  }

  /**
   * `items` ordered by `order`, stable: ids the order does not name keep their
   * relative place after the ones it does — an entry added since, say.
   */
  function inOrder<T extends { id: string }>(items: readonly T[], order: readonly string[] | undefined): T[] {
    if (!order || order.length === 0) return [...items];
    const rank = new Map(order.map((id, i) => [id, i]));
    return items
      .map((it, i) => ({ it, i, r: rank.get(it.id) ?? Number.MAX_SAFE_INTEGER }))
      .sort((a, b) => a.r - b.r || a.i - b.i)
      .map((x) => x.it);
  }

  /** Sections in the reader's order. Only the reader ever moves a section. */
  const orderedSections = $derived(inOrder(doc.sections, variant.section_order));

  const model = $derived.by((): SectionModel[] =>
    orderedSections
      // A skills section with no lines of its own still prints one — built
      // from the profile — and its skills can be left out like any other.
      .filter((s) => s.entries.length > 0 || (s.kind === "skills" && view.skills.length > 0))
      .map((section) => {
        const sectionOff = stateOf(section.id) === "out";
        // The reader's order where they set one, else the order the page drew
        // (the tailoring leads Projects and the like with what fits the job).
        const entryOrder = variant.order[section.id] ?? view.order[section.id];
        const entries = inOrder(section.entries, entryOrder).map((entry): EntryModel => {
          const off = sectionOff || stateOf(entry.id) === "out";
          const rows = entry.bullets.map((bullet): Row => {
            const state = stateOf(bullet.id);
            const shown =
              !off && (state === "in" || (state === "auto" && onPage.has(bullet.id)));
            return {
              bullet,
              state,
              shown,
              why: view.why[bullet.id] ?? [],
              echo: view.echo[bullet.id] ?? [],
            };
          });
          const names =
            section.kind === "skills" ? skillItems(entry.detail) : detailItems(entry.detail);
          const items = names.map((name): Item => {
            const key = section.kind === "skills" ? skillKey(name) : detailKey(entry.id, name);
            return { name, key, on: variant.include[key] !== false };
          });
          return {
            entry,
            items,
            off,
            rows,
            onCount: rows.filter((r) => r.shown).length,
            visible: rows.filter(passes),
          };
        });
        return {
          section,
          off: sectionOff,
          entries,
          onCount: entries.reduce((n, e) => n + e.onCount, 0),
          total: entries.reduce((n, e) => n + e.rows.length, 0),
        };
      }),
  );

  const totals = $derived.by(() => {
    let all = 0;
    let on = 0;
    for (const s of model) {
      all += s.total;
      on += s.onCount;
    }
    return { all, on, off: all - on };
  });

  /** What the outline draws under the current filter. */
  const shownModel = $derived(
    narrowing
      ? model
          .map((s) => ({ ...s, entries: s.entries.filter((e) => e.visible.length > 0) }))
          .filter((s) => s.entries.length > 0)
      : model,
  );

  const allIds = $derived(model.flatMap((s) => s.entries.map((e) => e.entry.id)));
  const anyOpen = $derived(allIds.some((id) => expanded.has(id)));

  function toggleAll() {
    if (anyOpen) expanded.clear();
    else for (const id of allIds) expanded.add(id);
  }

  function toggleOpen(id: string) {
    if (expanded.has(id)) expanded.delete(id);
    else expanded.add(id);
  }

  // --- reordering, for this posting only ---
  //
  // Dragging writes the whole order, so what is on screen is exactly what was
  // saved; the master résumé keeps its own. A filtered outline shows a subset,
  // and moving within a subset has no clear meaning, so the handles go away
  // while one is on.

  /** On, or left out of this version — one technology, one skill. */
  function toggleItem(item: Item) {
    const include = { ...variant.include };
    if (item.on) include[item.key] = false;
    else delete include[item.key];
    onVariant({ ...variant, include });
  }

  /** The profile-built skills block, when the résumé writes none of its own. */
  const generatedSkills = $derived(
    view.skills.map(([label, names]) => ({
      label,
      items: names.map((name): Item => {
        const key = skillKey(name);
        return { name, key, on: variant.include[key] !== false };
      }),
    })),
  );

  function moveSection(from: number, to: number) {
    const shown = moved(
      model.map((s) => s.section.id),
      from,
      to,
    );
    // Empty sections draw nothing and are not listed; they keep their place
    // after the ones that were.
    const rest = orderedSections.map((s) => s.id).filter((id) => !shown.includes(id));
    onVariant({ ...variant, section_order: [...shown, ...rest] });
  }

  function moveEntry(s: SectionModel, from: number, to: number) {
    const ids = moved(
      s.entries.map((e) => e.entry.id),
      from,
      to,
    );
    onVariant({ ...variant, order: { ...variant.order, [s.section.id]: ids } });
  }

  function pickSkill(s: string) {
    skill = skill === s ? null : s;
    if (skill) filter = "all";
  }

  /**
   * What an entry is called. A skills line names itself by its label rather
   * than an organisation, and `org || title` covers both.
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
    // Not the detail line: it is listed item by item when the entry opens,
    // and a copy here would still show what was switched off.
    const role = entry.org && entry.title ? entry.title : "";
    return [role, dates].filter(Boolean).join(" · ");
  }

  // --- rewording, for this posting only ---
  //
  // Nothing is ever reworded for you — there is no model here — but you may
  // reword a bullet yourself for one employer: their name for the stack, the
  // metric they care about. The master résumé is untouched.

  let editing = $state<string | null>(null);
  let draftText = $state("");

  function startEdit(id: string, current: string) {
    editing = id;
    draftText = current;
  }

  function finishEdit(id: string, original: string) {
    const next = draftText.trim();
    const text = { ...variant.text };
    if (!next || next === original.trim()) delete text[id];
    else text[id] = next;
    editing = null;
    onVariant({ ...variant, text });
  }

  /** The draft (or the standing rewording) becomes the résumé's own wording. */
  function keepEverywhere(id: string, text: string) {
    const next = text.trim();
    if (!next || !onSaveToResume) return;
    editing = null;
    onSaveToResume(id, next);
  }

  function revertText(id: string) {
    const text = { ...variant.text };
    delete text[id];
    editing = null;
    onVariant({ ...variant, text });
  }

  let headlineTimer: ReturnType<typeof setTimeout> | undefined;
  // The page keys this panel on the posting, so a timer still pending when it
  // goes would write into whichever posting replaced it. Dropped instead: at
  // most half a second of typing.
  $effect(() => () => clearTimeout(headlineTimer));
  function onHeadline(value: string) {
    clearTimeout(headlineTimer);
    headlineTimer = setTimeout(() => {
      const next = value.trim();
      const headline = !next || next === (doc.contact.headline ?? "").trim() ? null : next;
      if (headline !== variant.headline) onVariant({ ...variant, headline });
    }, 500);
  }

  const FILTERS: { id: Filter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "on", label: "On page" },
    { id: "off", label: "Left off" },
  ];
</script>

{#snippet box(checked: boolean)}
  <span
    class="flex h-4 w-4 items-center justify-center rounded-[4px] border transition-colors {checked
      ? 'border-brand bg-brand text-brand-fg'
      : 'border-line bg-surface group-hover/box:border-brand'}"
  >
    {#if checked}<Check size={11} strokeWidth={3.25} />{/if}
  </span>
{/snippet}

{#snippet chips(items: Item[], disabled: boolean, what: string)}
  <div class="flex flex-wrap gap-1" role="group" aria-label={what}>
    {#each items as item (item.key)}
      <button
        onclick={() => toggleItem(item)}
        aria-pressed={item.on}
        {disabled}
        title={item.on ? `Leave ${item.name} off this version` : `Put ${item.name} back`}
        class="inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11.5px] font-medium transition-colors disabled:opacity-40 {item.on
          ? 'border-line bg-surface text-foreground hover:border-brand'
          : 'border-dashed border-line text-subtle line-through decoration-1 hover:text-foreground'}"
      >
        {#if item.on}<Check size={10} strokeWidth={3} class="text-brand" />{/if}
        {item.name}
      </button>
    {/each}
  </div>
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  <!-- Filters stay put while the outline scrolls under them. -->
  <div class="flex shrink-0 flex-wrap items-center gap-2 border-b border-line px-3 py-2">
    <div class="flex items-center rounded-md border border-line bg-surface p-0.5" role="group" aria-label="Show">
      {#each FILTERS as f (f.id)}
        {@const n = f.id === "all" ? totals.all : f.id === "on" ? totals.on : totals.off}
        <button
          onclick={() => (filter = f.id)}
          aria-pressed={filter === f.id}
          class="h-6 rounded px-2 text-[12px] font-medium transition-colors {filter === f.id
            ? 'bg-hover text-foreground'
            : 'text-subtle hover:text-foreground'}"
        >
          {f.label}
          <span class="ml-0.5 tabular-nums opacity-70">{n}</span>
        </button>
      {/each}
    </div>
    {#if skill}
      <button
        onclick={() => (skill = null)}
        class="inline-flex h-7 items-center gap-1 rounded-md bg-brand/10 px-2 text-[12px] font-medium text-brand"
        aria-label="Stop filtering by {skill}"
      >
        Names {skill}
        <X size={12} />
      </button>
    {/if}
    <button
      onclick={toggleAll}
      class="tool ml-auto"
      title={anyOpen ? "Fold every entry" : "Open every entry"}
      aria-label={anyOpen ? "Collapse all" : "Expand all"}
    >
      {#if anyOpen}<ChevronsDownUp size={14} />{:else}<ChevronsUpDown size={14} />{/if}
    </button>
  </div>

  <div class="pane flex-1 space-y-4 px-3 py-3">
    <!-- What the posting asks for: one line until it is wanted. -->
    {#if byHand}
      <p class="flex items-start gap-2 rounded-md bg-line-soft px-3 py-2.5 text-[12.5px] text-muted">
        <ListChecks size={14} class="mt-0.5 shrink-0 text-subtle" />
        No job behind this version — tick what you want on it. Nothing is cut or moved for you; if
        it runs past the page limit, the preview says so.
      </p>
    {:else if view.required.length === 0}
      <p class="flex items-start gap-2 rounded-md bg-line-soft px-3 py-2.5 text-[12.5px] text-muted">
        <Target size={14} class="mt-0.5 shrink-0 text-subtle" />
        This posting's page could not be read, so there are no skills to tailor against. Your full
        résumé is trimmed to the page limit only.
      </p>
    {:else}
      <section class="card">
        <button
          onclick={() => (matchOpen = !matchOpen)}
          aria-expanded={matchOpen}
          class="flex w-full items-center gap-3 px-3 py-2.5 text-left"
        >
          <Target size={15} class="shrink-0 text-brand" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-[13px]">
              <span class="font-semibold">{view.covered.length} of {view.required.length}</span>
              <span class="text-muted">skills it asks for are on the page</span>
              {#if split}
                <span class="text-subtle">· {mustCovered}/{view.must.length} must-haves</span>
              {/if}
            </p>
            <div class="mt-1.5 h-1 overflow-hidden rounded-full bg-line-soft">
              <div class="h-full rounded-full bg-job transition-[width]" style="width: {coverage * 100}%"></div>
            </div>
          </div>
          <ChevronDown
            size={15}
            class="shrink-0 text-subtle transition-transform {matchOpen ? 'rotate-180' : ''}"
          />
        </button>
        {#if matchOpen}
          <div class="border-t border-line-soft px-3 py-3">
            <div class="flex flex-wrap gap-1.5">
              {#each ordered as s (s)}
                {@const covered = coveredSet.has(s)}
                {@const optional = split && !mustSet.has(s)}
                <button
                  onclick={() => pickSkill(s)}
                  aria-pressed={skill === s}
                  title={`${optional ? "Nice to have. " : ""}Show the bullets that name ${s}`}
                  class="inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11.5px] font-medium transition-colors {optional
                    ? 'border-dashed'
                    : ''} {skill === s
                    ? 'border-brand bg-brand text-brand-fg'
                    : covered
                      ? 'border-transparent bg-job-soft text-job hover:border-job/40'
                      : 'border-line text-subtle hover:border-brand hover:text-foreground'}"
                >
                  {#if covered && skill !== s}<Check size={11} strokeWidth={3} />{/if}
                  {s}
                </button>
              {/each}
            </div>
            <p class="mt-2.5 text-[11.5px] text-subtle">
              Click a skill to list the bullets that name it.{#if split}
                A dashed one is optional.{/if}
            </p>
            {#if view.focus.length > 0}
              <p class="mt-2 text-[12px] text-muted">
                It keeps coming back to
                <span class="font-medium text-foreground">{view.focus.join(", ")}</span> — bullets
                about those are kept first.
              </p>
            {/if}
            {#if missing.length > 0}
              <p class="mt-2 text-[12px] text-muted">
                Nothing in your résumé names {missing.length === 1 ? "the grey one" : "the grey ones"}.
                If you have done that work, add a bullet that says so on your résumé.
              </p>
            {/if}
          </div>
        {/if}
      </section>
    {/if}

    <label class="block">
      <span class="eyebrow mb-1 block">Headline for this {byHand ? "version" : "posting"}</span>
      <input
        value={variant.headline ?? ""}
        oninput={(e) => onHeadline(e.currentTarget.value)}
        placeholder={doc.contact.headline || "Optional — the line under your name"}
        maxlength="120"
        class="control control-focus w-full text-[13px]"
      />
    </label>

    {#if frozen && over}
      <div class="flex items-start gap-2 rounded-md border border-star/30 bg-star-soft/50 px-3 py-2 text-[12px]">
        <CircleAlert size={13} class="mt-0.5 shrink-0 text-star" />
        <p class="min-w-0 flex-1 text-muted">
          <span class="font-semibold text-foreground">
            Runs to {view.pages.length} pages — the limit is {view.theme.max_pages}.
          </span>
          Untick a bullet or two, or raise the limit under Design. Nothing is cut for you.
        </p>
      </div>
    {:else if frozen && !byHand}
      <p class="px-1 text-[11.5px] text-subtle">
        The app picked these the first time you opened this posting. From now on only your ticks
        change the page — Start over in the toolbar lets it pick again.
      </p>
    {/if}

    {#if !frozen && view.dropped.length > 0 && filter !== "off"}
      <div class="flex items-start gap-2 rounded-md border border-star/30 bg-star-soft/50 px-3 py-2 text-[12px]">
        <CircleAlert size={13} class="mt-0.5 shrink-0 text-star" />
        <p class="min-w-0 flex-1 text-muted">
          <span class="font-semibold text-foreground">
            {view.dropped.length} {view.dropped.length === 1 ? "bullet" : "bullets"} cut to fit the page.
          </span>
          Tick one to keep it; the weakest of the rest gives way.
        </p>
        <button onclick={() => (filter = "off")} class="shrink-0 font-semibold text-brand hover:underline">
          Show
        </button>
      </div>
    {/if}

    {#if shownModel.length === 0}
      <div class="py-10 text-center text-[13px] text-subtle">
        <p>Nothing matches.</p>
        <button
          onclick={() => {
            filter = "all";
            skill = null;
          }}
          class="mt-2 font-semibold text-brand hover:underline"
        >
          Show everything
        </button>
      </div>
    {/if}

    {#each shownModel as s (s.section.id)}
      <section aria-label={s.section.title} data-sort-item class="rounded-md data-[sorting]:bg-background data-[sorting]:shadow-lg">
        <div class="mb-1.5 flex items-center gap-2 px-1">
          {#if !narrowing && shownModel.length > 1}
            <button
              use:sortHandle={{ onMove: moveSection }}
              aria-label="Move the {s.section.title} section"
              title="Drag to move this section · ↑/↓ on the keyboard"
              class="-ml-1 flex h-5 w-4 shrink-0 cursor-grab touch-none items-center justify-center text-subtle/70 hover:text-foreground active:cursor-grabbing"
            >
              <GripVertical size={13} />
            </button>
          {/if}
          <button
            role="checkbox"
            aria-checked={!s.off}
            aria-label="Include the {s.section.title} section"
            title={s.off ? "Put this section back" : "Leave this whole section off"}
            onclick={() => toggleBlock(s.section.id)}
            class="group/box shrink-0"
          >
            {@render box(!s.off)}
          </button>
          <h3 class="eyebrow min-w-0 flex-1 truncate {s.off ? 'line-through' : ''}">
            {s.section.title}
          </h3>
          {#if s.total > 0}
            <span class="shrink-0 text-[11px] text-subtle tabular-nums">
              {s.onCount} of {s.total} on page
            </span>
          {/if}
        </div>

        <div class="card divide-y divide-line-soft overflow-hidden {s.off ? 'opacity-60' : ''}">
          {#if s.section.kind === "skills" && s.entries.length === 0}
            <p class="px-3 pt-2.5 pb-1 text-[11.5px] text-subtle">
              Built from the skills on your profile. Untick any you don't want on this version.
            </p>
            {#each generatedSkills as line (line.label)}
              <div class="px-3 py-2">
                <p class="mb-1 text-[12px] font-semibold">{line.label}</p>
                {@render chips(line.items, s.off, `${line.label} skills`)}
              </div>
            {/each}
          {/if}
          {#each s.entries as e (e.entry.id)}
            {@const entryOut = stateOf(e.entry.id) === "out"}
            {@const open = narrowing || expanded.has(e.entry.id)}
            {@const sub = entrySub(e.entry)}
            <div data-sort-item class="bg-surface data-[sorting]:shadow-lg">
              <div class="flex items-center gap-2.5 py-2 pr-3 pl-1.5">
                {#if !narrowing && s.entries.length > 1}
                  <button
                    use:sortHandle={{ onMove: (from, to) => moveEntry(s, from, to) }}
                    aria-label="Move {entryLabel(e.entry)}"
                    title="Drag to move · ↑/↓ on the keyboard"
                    class="flex h-6 w-4 shrink-0 cursor-grab touch-none items-center justify-center text-subtle/70 hover:text-foreground active:cursor-grabbing"
                  >
                    <GripVertical size={13} />
                  </button>
                {:else}
                  <span class="w-4 shrink-0"></span>
                {/if}
                <button
                  role="checkbox"
                  aria-checked={!e.off}
                  aria-label="Include {entryLabel(e.entry)}"
                  title={entryOut ? "Put this back on the page" : "Leave this off for this posting"}
                  disabled={s.off}
                  onclick={() => toggleBlock(e.entry.id)}
                  class="group/box shrink-0 disabled:opacity-40"
                >
                  {@render box(!e.off)}
                </button>
                <button
                  onclick={() => toggleOpen(e.entry.id)}
                  aria-expanded={e.rows.length > 0 || e.items.length > 0 ? open : undefined}
                  disabled={(e.rows.length === 0 && e.items.length === 0) || narrowing}
                  class="flex min-w-0 flex-1 items-center gap-2 text-left"
                >
                  <span class="min-w-0 flex-1">
                    <span
                      class="block truncate text-[13px] font-semibold {e.off
                        ? 'text-subtle line-through decoration-1'
                        : ''}"
                    >
                      {entryLabel(e.entry)}
                    </span>
                    {#if sub}
                      <span class="block truncate text-[11.5px] text-subtle">{sub}</span>
                    {/if}
                  </span>
                  {#if e.rows.length === 0 && e.items.length > 0}
                    {@const onItems = e.items.filter((i) => i.on).length}
                    <span
                      class="shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium tabular-nums {onItems === 0
                        ? 'text-subtle'
                        : 'bg-hover text-muted'}"
                      title="{onItems} of {e.items.length} on the page"
                    >
                      {onItems}/{e.items.length}
                    </span>
                    {#if !narrowing}
                      <ChevronDown
                        size={14}
                        class="shrink-0 text-subtle transition-transform {open ? '' : '-rotate-90'}"
                      />
                    {/if}
                  {/if}
                  {#if e.rows.length > 0}
                    <span
                      class="shrink-0 rounded px-1.5 py-0.5 text-[11px] font-medium tabular-nums {e.onCount ===
                      0
                        ? 'text-subtle'
                        : 'bg-hover text-muted'}"
                      title="{e.onCount} of {e.rows.length} bullets on the page"
                    >
                      {e.onCount}/{e.rows.length}
                    </span>
                    {#if !narrowing}
                      <ChevronDown
                        size={14}
                        class="shrink-0 text-subtle transition-transform {open ? '' : '-rotate-90'}"
                      />
                    {/if}
                  {/if}
                </button>
              </div>

              {#if open && !narrowing && e.items.length > 0}
                <div class="pr-3 pb-2 pl-[3.625rem] {e.off ? 'opacity-50' : ''}">
                  <p class="mb-1 text-[11px] text-subtle">
                    {s.section.kind === "skills"
                      ? "Skills on this line"
                      : ["projects", "experience", "leadership"].includes(s.section.kind)
                        ? "Skills & tools"
                        : "Detail line"}
                  </p>
                  {@render chips(e.items, e.off, `${entryLabel(e.entry)} details`)}
                </div>
              {/if}

              {#if open && e.visible.length > 0}
                <ul class="pb-1.5 {e.off ? 'opacity-50' : ''}">
                  {#each e.visible as row (row.bullet.id)}
                    {@const id = row.bullet.id}
                    {@const reworded =
                      variant.text[id] !== undefined && variant.text[id].trim() !== row.bullet.text.trim()}
                    <li class="group flex items-start gap-2.5 py-1.5 pr-2 pl-[3.625rem] hover:bg-hover/60">
                      {#if editing === id}
                        <div class="min-w-0 flex-1">
                          <textarea
                            bind:value={draftText}
                            rows="3"
                            aria-label="Reworded bullet for this posting"
                            onkeydown={(ev) => {
                              if (ev.key === "Escape") {
                                ev.stopPropagation();
                                editing = null;
                              } else if (ev.key === "Enter" && (ev.metaKey || ev.ctrlKey)) {
                                finishEdit(id, row.bullet.text);
                              }
                            }}
                            class="control control-focus block w-full resize-y text-[13px] leading-snug"
                          ></textarea>
                          <div class="mt-1.5 flex items-center gap-1.5">
                            <button onclick={() => finishEdit(id, row.bullet.text)} class="btn-primary h-7 px-2.5 text-[12px]">
                              Done
                            </button>
                            <button onclick={() => (editing = null)} class="tool tool-outline h-7 text-[12px]">
                              Cancel
                            </button>
                            {#if onSaveToResume}
                              <button
                                onclick={() => keepEverywhere(id, draftText)}
                                disabled={!draftText.trim() || draftText.trim() === row.bullet.text.trim()}
                                class="tool tool-outline h-7 text-[12px] disabled:opacity-40"
                                title="Write this wording into your résumé itself, for every version and posting"
                              >
                                <Save size={12} /> Save to résumé
                              </button>
                            {/if}
                            {#if reworded}
                              <button onclick={() => revertText(id)} class="tool h-7 text-[12px]">
                                Use original
                              </button>
                            {/if}
                            <span class="ml-auto text-[11px] text-subtle">Done keeps it to this posting</span>
                          </div>
                        </div>
                      {:else}
                        <button
                          role="checkbox"
                          aria-checked={row.shown}
                          disabled={e.off}
                          onclick={() => setInclude(id, row.shown ? "out" : "in")}
                          aria-label={row.shown ? "Leave this bullet off" : "Keep this bullet on the page"}
                          title={row.shown
                            ? "Leave this bullet off"
                            : frozen
                              ? "Put this bullet on the page"
                              : "Keep this bullet — it will not be cut to fit"}
                          class="group/box mt-[3px] shrink-0 disabled:opacity-40"
                        >
                          {@render box(row.shown)}
                        </button>
                        <div class="min-w-0 flex-1">
                          <p class="text-[13px] leading-snug {row.shown ? 'text-foreground' : 'text-subtle'}">
                            {variant.text[id] ?? row.bullet.text}
                          </p>
                          {#if (!frozen && row.state !== "auto") || reworded || row.why.length > 0 || row.echo.length > 0 || droppedIds.has(id)}
                            <!-- Two kinds of evidence, drawn apart: a skill the
                                 posting named, and the posting's own words the
                                 bullet happens to echo — weaker, and not coverage. -->
                            <p class="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px]">
                              {#if frozen}
                                <!-- Every tick is the reader's own now; a label
                                     on each would only repeat the checkbox. -->
                              {:else if row.state === "in"}
                                <span class="inline-flex items-center gap-0.5 font-semibold text-brand">
                                  <Pin size={10} /> Pinned
                                </span>
                              {:else if row.state === "out"}
                                <span class="inline-flex items-center gap-0.5 font-medium text-muted">
                                  <EyeOff size={10} /> Left off
                                </span>
                              {:else if !row.shown && droppedIds.has(id)}
                                <span class="font-medium text-star">Cut to fit</span>
                              {/if}
                              {#if reworded}
                                <span class="font-medium text-brand">Reworded</span>
                              {/if}
                              {#if row.why.length > 0}
                                <span class="font-medium text-job">{row.why.join(" · ")}</span>
                              {/if}
                              {#if row.echo.length > 0}
                                <span class="text-subtle" title="Words this posting uses that the bullet echoes">
                                  {row.echo.join(", ")}
                                </span>
                              {/if}
                            </p>
                          {/if}
                        </div>
                        <div class="reveal-on-hover flex shrink-0 items-center">
                          <button
                            onclick={() => startEdit(id, variant.text[id] ?? row.bullet.text)}
                            disabled={e.off}
                            aria-label="Reword this bullet for this posting"
                            title="Reword for this posting"
                            class="tool h-7 min-w-7 px-0 disabled:opacity-40"
                          >
                            <Pencil size={13} />
                          </button>
                          {#if reworded && onSaveToResume}
                            <button
                              onclick={() => keepEverywhere(id, variant.text[id] ?? "")}
                              disabled={e.off}
                              aria-label="Save this wording to your résumé"
                              title="Save this wording to your résumé — every version gets it"
                              class="tool h-7 min-w-7 px-0 disabled:opacity-40"
                            >
                              <Save size={13} />
                            </button>
                          {/if}
                          {#if !frozen && row.state !== "auto"}
                            <button
                              onclick={() => setInclude(id, "auto")}
                              aria-label="Let the app decide this bullet"
                              title="Let the app decide"
                              class="tool h-7 min-w-7 px-0"
                            >
                              <Undo2 size={13} />
                            </button>
                          {/if}
                        </div>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/each}
  </div>
</div>
