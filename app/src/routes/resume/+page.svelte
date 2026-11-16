<script lang="ts">
  import PageHeader from "$lib/components/PageHeader.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Field from "$lib/components/resume/Field.svelte";
  import EntryEditor from "$lib/components/resume/EntryEditor.svelte";
  import ImportPanel from "$lib/components/resume/ImportPanel.svelte";
  import ResumePreview from "$lib/components/resume/ResumePreview.svelte";
  import ResumeSwitcher from "$lib/components/resume/ResumeSwitcher.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import { toast } from "$lib/toast.svelte";
  import ThemePicker from "$lib/components/resume/ThemePicker.svelte";
  import { tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    ChevronRight,
    ChevronsDownUp,
    ChevronsUpDown,
    Download,
    FileText,
    Loader,
    Palette,
    Plus,
    RotateCcw,
    Target,
    Trash2,
    Upload,
  } from "$lib/icons";
  import { resume, emptyEntry } from "$lib/resume.svelte";
  import { feed } from "$lib/feed.svelte";
  import { outcomeMessage, saveResumeJson, saveResumePdf } from "$lib/resumeFile";
  import type { ResumeSection } from "$lib/api";

  /**
   * The résumé builder: the master document, edited once, previewed live.
   *
   * The preview is not a mock-up of the PDF — it is the same positioned boxes
   * Rust hands the PDF writer, drawn as SVG. Whatever this shows is what the
   * file contains, including where every line wraps and whether it fits a page.
   *
   * There is no save button. Nothing else in this app has one either: edits go
   * to the store, which relays out after a beat and writes after a longer one.
   */

  let tab = $state<"content" | "design">("content");

  // --- getting around a long résumé ---
  //
  // Entries start folded to a line each and sections can fold too, so the
  // page reads as an outline; the bar over it jumps to a section and says
  // which one is in view. `fold` is "open all / fold all": every entry editor
  // follows a change of its counter, and otherwise keeps its own state.
  let fold = $state({ open: false, n: 0 });
  const foldedSections = new SvelteSet<string>();
  /** The entry just added, which arrives open: there is nothing to fold yet. */
  let added = $state<{ section: string; index: number } | null>(null);
  let pane = $state<HTMLDivElement | null>(null);
  let inView = $state("you");

  const sectionKey = (section: ResumeSection) => section.id || section.kind;

  function toggleEverything() {
    const open = !fold.open;
    fold = { open, n: fold.n + 1 };
    if (open) foldedSections.clear();
  }

  function toggleSection(key: string) {
    if (foldedSections.has(key)) foldedSections.delete(key);
    else foldedSections.add(key);
  }

  async function jump(key: string) {
    foldedSections.delete(key);
    await tick();
    const el = document.getElementById(`resume-${key}`);
    if (!el || !pane) return;
    // Straight there, under the bar. A smooth scroll is not reliable in every
    // webview — in some it never starts — and a jump that goes nowhere is worse
    // than one without an animation.
    const nav = pane.querySelector<HTMLElement>("nav[aria-label='Résumé sections']");
    const offset = (nav?.offsetHeight ?? 48) + 12;
    pane.scrollTo({
      top: pane.scrollTop + el.getBoundingClientRect().top - pane.getBoundingClientRect().top - offset,
    });
    inView = key;
  }

  /** The last section whose top has passed just under the jump bar. */
  function spy() {
    if (!pane || tab !== "content") return;
    const line = pane.getBoundingClientRect().top + 96;
    let current = "you";
    for (const el of pane.querySelectorAll<HTMLElement>("[data-resume-section]")) {
      if (el.getBoundingClientRect().top <= line) current = el.dataset.resumeSection ?? current;
    }
    inView = current;
  }
  let importing = $state(false);
  let importText = $state("");
  /** What is running, so each button spins for its own job and not the others'. */
  let busy = $state<null | "pdf" | "json" | "import" | "reset" | "create">(null);

  /** A file saved, or why not. A cancelled dialog says nothing. */
  function report(outcome: Parameters<typeof outcomeMessage>[0]) {
    const message = outcomeMessage(outcome);
    if (!message) return;
    if (outcome.ok) toast.success(message);
    else toast.error("save the file", message);
  }

  // Once per session, not once per visit: coming back to the builder must show
  // the document as the user left it, including edits the debounced save has
  // not written yet. `resume.ensure` carries the reasoning.
  $effect(() => {
    void resume.ensure();
  });

  const doc = $derived(resume.doc);
  const view = $derived(resume.view);
  function touch() {
    resume.touch();
  }

  function addEntry(section: ResumeSection) {
    added = { section: sectionKey(section), index: section.entries.length };
    foldedSections.delete(sectionKey(section));
    section.entries = [...section.entries, emptyEntry()];
    // Read once, when the new editor mounts; cleared so no later editor that
    // happens to land at that index opens too.
    void tick().then(() => (added = null));
    touch();
  }

  function removeEntry(section: ResumeSection, at: number) {
    section.entries = section.entries.filter((_, i) => i !== at);
    touch();
  }

  function moveEntry(section: ResumeSection, at: number, delta: number) {
    const to = at + delta;
    if (to < 0 || to >= section.entries.length) return;
    const next = [...section.entries];
    [next[at], next[to]] = [next[to], next[at]];
    section.entries = next;
    touch();
  }

  function addLink() {
    if (!doc) return;
    doc.contact.links = [...doc.contact.links, { label: "GitHub", url: "" }];
    touch();
  }

  function removeLink(at: number) {
    if (!doc) return;
    doc.contact.links = doc.contact.links.filter((_, i) => i !== at);
    touch();
  }

  async function downloadPdf() {
    if (!view) return;
    busy = "pdf";
    try {
      // Flush any pending debounce first, or the file could be a beat behind
      // the preview the user is looking at.
      await resume.save();
      report(
        await saveResumePdf({
          id: resume.id ?? undefined,
          theme: resume.theme ?? undefined,
          filename: view.filename,
          reveal: true,
        }),
      );
    } finally {
      busy = null;
    }
  }

  async function exportJson() {
    busy = "json";
    try {
      await resume.save();
      report(await saveResumeJson(resume.id ?? undefined, resume.name));
    } finally {
      busy = null;
    }
  }

  /** Empties the résumé, after asking. */
  async function resetResume() {
    const ok = await confirm({
      title: `Erase everything in “${resume.name}”?`,
      body: "Every section is emptied — contact details, experience, projects, skills. The design is kept. This cannot be undone, so export a resume.json first if anything here is worth keeping.",
      confirmLabel: "Erase résumé",
      danger: true,
    });
    if (!ok) return;
    busy = "reset";
    try {
      await resume.reset();
      toast.info("Erased. The headings are still there to fill in again.");
    } finally {
      busy = null;
    }
  }

  async function runImport() {
    if (!importText.trim()) return;
    busy = "import";
    try {
      await resume.importJson(importText);
      importing = false;
      importText = "";
      toast.success(`Imported as “${resume.name}”`);
    } catch (e) {
      toast.error("import that résumé", e);
    } finally {
      busy = null;
    }
  }

  async function startNew() {
    busy = "create";
    try {
      await resume.create();
    } catch (e) {
      toast.error("start a résumé", e);
    } finally {
      busy = null;
    }
  }
</script>

<svelte:head><title>Résumé · Anti-FOMO</title></svelte:head>

<div class="flex h-full min-h-0 flex-col">
  <PageHeader
    title="Résumé"
    subtitle="Write it once. Every role you open can tailor a copy of it to what that posting asks for."
  >
    {#snippet actions()}
      {#if resume.exists}
        <!-- The sidebar has this too; phones have no sidebar. -->
        <a href="/resume/tailor" class="tool mr-1" title="Tailor this résumé to a job">
          <Target size={14} /><span class="hidden sm:inline">Tailor for a job</span>
        </a>
        <button
          onclick={downloadPdf}
          disabled={busy === "pdf" || !view}
          class="inline-flex items-center gap-1.5 rounded-md bg-brand px-3.5 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
        >
          {#if busy === "pdf"}<Loader size={15} class="animate-spin" />{:else}<Download size={15} />{/if}
          Save PDF
        </button>
      {/if}
    {/snippet}
  </PageHeader>

  <div class="pane flex-1" bind:this={pane} onscroll={spy}>
  <div class="mx-auto w-full max-w-7xl px-4 py-5 sm:px-6">

  {#if resume.error && resume.exists}
    <p class="mt-3 rounded-md bg-danger-soft p-3 text-sm text-danger" role="alert">
      {resume.error}
    </p>
  {/if}

  {#if resume.loading}
    <div class="mt-10 flex justify-center" aria-busy="true" aria-label="Loading the résumé">
      <Loader size={22} class="animate-spin text-subtle" />
    </div>
  {:else if !resume.exists && resume.error}
    <!-- A read that failed is not "no résumé yet": offering to start one here
         would create a second row beside the one that could not be read. -->
    <div class="mt-4">
      <EmptyState
        icon={FileText}
        title="The résumé could not be loaded"
        body={resume.error}
        actionLabel="Try again"
        onAction={() => void resume.load()}
      />
    </div>
  {:else if !resume.exists}
    <div class="mt-4">
      <EmptyState
        icon={FileText}
        title="No résumé yet"
        body="Start an empty one and fill in your experience, or bring in what you already have — a resume.json, or the CVs and notes sitting in a chat thread."
        actionLabel="Start a résumé"
        onAction={() => void startNew()}
        busy={busy === "create"}
      />
      <div class="mt-3 text-center">
        <button
          onclick={() => (importing = !importing)}
          class="inline-flex items-center gap-1.5 text-sm font-semibold text-brand hover:underline"
        >
          <Upload size={14} />
          Bring in an existing résumé
        </button>
      </div>
      {#if importing}
        <div class="card mx-auto mt-4 max-w-2xl p-5">
          <ImportPanel bind:text={importText} busy={busy === "import"} onImport={runImport} />
        </div>
      {/if}
    </div>
  {:else if doc}
    <div class="mt-4 lg:grid lg:grid-cols-[minmax(0,1fr)_minmax(0,26rem)] lg:gap-6">
      <!-- Editor. Room below the last section, so a jump to it can bring it
           to the top rather than stopping short with the one above in view. -->
      <div class="min-w-0 {tab === 'content' ? 'lg:pb-[45dvh]' : ''}">
        <div class="mb-4 flex flex-wrap items-center gap-2">
          <ResumeSwitcher />
          {#each [{ id: "content", label: "Content", icon: FileText }, { id: "design", label: "Design", icon: Palette }] as const as t (t.id)}
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
          <span class="ml-auto text-xs text-subtle">
            {#if resume.saving}Saving…{:else if resume.dirty}Unsaved{:else}Saved{/if}
          </span>
        </div>

        {#if tab === "content"}
          <!-- Sticks to the top of the pane: where you are, and a way to the rest. -->
          <nav
            class="sticky top-0 z-10 -mx-1 mb-4 flex items-center gap-1 border-b border-line bg-background/95 px-1 py-2 backdrop-blur"
            aria-label="Résumé sections"
          >
            <div class="scrollbar-hide flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
              {#each [{ key: "you", label: "You", count: null as number | null }, ...doc.sections.map( (sec) => ({ key: sectionKey(sec), label: sec.title || "Untitled", count: sec.entries.length as number | null }), )] as item (item.key)}
                <button
                  onclick={() => jump(item.key)}
                  aria-current={inView === item.key ? "location" : undefined}
                  class="inline-flex h-7 shrink-0 items-center gap-1 rounded-md px-2.5 text-[12px] font-medium transition-colors {inView ===
                  item.key
                    ? 'bg-hover text-foreground'
                    : 'text-subtle hover:text-foreground'}"
                >
                  {item.label}
                  {#if item.count}
                    <span class="tabular-nums opacity-60">{item.count}</span>
                  {/if}
                </button>
              {/each}
            </div>
            <button
              onclick={toggleEverything}
              class="tool shrink-0"
              title={fold.open ? "Fold every entry to one line" : "Open every entry"}
            >
              {#if fold.open}<ChevronsDownUp size={14} />{:else}<ChevronsUpDown size={14} />{/if}
              <span class="hidden sm:inline">{fold.open ? "Fold all" : "Open all"}</span>
            </button>
          </nav>
        {/if}

        {#if tab === "design"}
          <section class="card p-4">
            {#if resume.theme}
              <ThemePicker
                theme={resume.theme}
                options={resume.themes}
                onChange={(next) => resume.setTheme(next)}
              />
            {/if}
          </section>

          <section class="card mt-4 border-danger/30 p-4">
            <h2 class="mb-1 text-[14px] font-semibold">Start over</h2>
            <p class="mb-4 text-sm text-muted">
              Empties every section — contact details, experience, projects, skills — and leaves
              the blank headings behind. The design is kept. This cannot be undone, so export a
              resume.json from Content first if there is anything in here worth keeping.
            </p>

            <button
              onclick={resetResume}
              disabled={busy === "reset"}
              class="inline-flex items-center gap-1.5 rounded-md border border-danger/40 px-4 py-2 text-sm font-semibold text-danger transition-colors hover:bg-danger hover:text-danger-fg disabled:opacity-50"
            >
              <RotateCcw size={15} />
              {busy === "reset" ? "Erasing…" : "Reset résumé"}
            </button>
          </section>
        {:else}
          <!-- Backup sits with the content it carries: an export is the one copy
               that is not on this device, and an import is content arriving. -->
          <section class="card mb-4 p-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div class="min-w-0">
                <h2 class="text-[14px] font-semibold">Backup & transfer</h2>
                <p class="mt-0.5 text-xs text-subtle">
                  Everything is on this device only. An export is the one copy that is not.
                </p>
              </div>
              <div class="flex shrink-0 flex-wrap gap-2">
                <button
                  onclick={exportJson}
                  disabled={busy === "json"}
                  class="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-2 text-sm font-semibold hover:border-brand disabled:opacity-50"
                >
                  {#if busy === "json"}<Loader size={14} class="animate-spin" />{:else}<Download size={14} />{/if}
                  Export resume.json
                </button>
                <button
                  onclick={() => (importing = !importing)}
                  aria-expanded={importing}
                  class="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-2 text-sm font-semibold hover:border-brand"
                >
                  <Upload size={14} />
                  Import
                </button>
              </div>
            </div>
            {#if importing}
              <div class="mt-4 border-t border-line-soft pt-4">
                <ImportPanel
                  bind:text={importText}
                  busy={busy === "import"}
                  onImport={runImport}
                  hint="Imports as a separate résumé and opens it. Export the one you have first if you want it back."
                />
              </div>
            {/if}
          </section>

          <section class="card mb-4 p-4" id="resume-you" data-resume-section="you">
            <h2 class="mb-3 text-[14px] font-semibold">You</h2>
            <div class="grid gap-3 sm:grid-cols-2">
              <Field
                label="Full name" autocomplete="name"
                bind:value={doc.contact.name}
                oninput={touch}
                placeholder="Ada Lovelace"
              />
              <Field
                label="Headline"
                bind:value={
                  () => doc.contact.headline ?? "",
                  (v) => (doc.contact.headline = v.trim() ? v : null)
                }
                oninput={touch}
                placeholder="Software Engineer"
              />
              <Field
                label="Email" type="email" autocomplete="email"
                bind:value={doc.contact.email}
                oninput={touch}
                placeholder="you@example.com"
              />
              <Field
                label="Phone" type="tel" autocomplete="tel"
                bind:value={doc.contact.phone}
                oninput={touch}
                placeholder="+1 416 555 0100"
              />
              <Field
                label="Location"
                bind:value={doc.contact.location}
                oninput={touch}
                placeholder="Toronto, ON"
              />
            </div>

            <p class="mt-4 mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Links</p>
            {#each doc.contact.links as link, i (i)}
              <div class="mb-2 flex items-end gap-2">
                <div class="w-32 shrink-0">
                  <Field label="Label" bind:value={link.label} oninput={touch} placeholder="GitHub" />
                </div>
                <div class="min-w-0 flex-1">
                  <Field
                    label="URL" type="url" autocomplete="off"
                    bind:value={link.url}
                    oninput={touch}
                    placeholder="https://github.com/you"
                  />
                </div>
                <button onclick={() => removeLink(i)} aria-label="Remove link" class="tap mb-0.5 h-9 w-9 hover:text-danger">
                  <Trash2 size={15} />
                </button>
              </div>
            {/each}
            <button
              onclick={addLink}
              class="inline-flex items-center gap-1 text-xs font-semibold text-brand hover:underline"
            >
              <Plus size={13} strokeWidth={2.5} />
              Add a link
            </button>
            <p class="mt-2 text-xs text-subtle">
              Printed without the <code>https://</code> — “github.com/you” — and clickable in the PDF.
            </p>
          </section>

          {#each doc.sections as section (section.id || section.kind)}
            {@const key = sectionKey(section)}
            {@const folded = foldedSections.has(key)}
            <section
              class="card mb-4 p-4"
              id="resume-{key}"
              data-resume-section={key}
            >
              <div class="flex items-center justify-between gap-2 {folded ? '' : 'mb-3'}">
                <button
                  onclick={() => toggleSection(key)}
                  aria-expanded={!folded}
                  aria-label="{folded ? 'Open' : 'Fold'} {section.title || 'this section'}"
                  class="tap h-7 w-7 shrink-0"
                >
                  <ChevronRight size={15} class="transition-transform {folded ? '' : 'rotate-90'}" />
                </button>
                <input
                  bind:value={section.title}
                  oninput={touch}
                  aria-label="Section name"
                  class="-mx-1 min-w-0 flex-1 rounded-md border-0 bg-transparent px-1 text-[14px] font-semibold hover:bg-line-soft focus-visible:bg-line-soft"
                />
                <span class="shrink-0 text-xs text-subtle">
                  {section.entries.length}
                  {section.entries.length === 1 ? "entry" : "entries"}
                </span>
              </div>

              {#if !folded}
              {#if section.kind === "skills"}
                <p class="mb-3 text-sm text-muted">
                  Leave this empty and the skills line is built from the skills you picked in
                  Settings, with whatever the posting asks for leading. Add a line here to write it
                  yourself instead.
                </p>
              {/if}

              <div class="space-y-3">
                {#each section.entries as entry, i (entry.id || i)}
                  <EntryEditor
                    bind:entry={section.entries[i]}
                    kind={section.kind}
                    index={i}
                    count={section.entries.length}
                    onChange={touch}
                    onRemove={() => removeEntry(section, i)}
                    onMove={(d) => moveEntry(section, i, d)}
                    startOpen={added?.section === key && added.index === i}
                    {fold}
                  />
                {/each}
              </div>

              <button
                onclick={() => addEntry(section)}
                class="mt-3 inline-flex items-center gap-1.5 text-sm font-semibold text-brand hover:underline"
              >
                <Plus size={14} strokeWidth={2.5} />
                Add {section.kind === "skills" ? "a skills line" : "an entry"}
              </button>
              {/if}
            </section>
          {/each}
        {/if}
      </div>

      <!-- Preview. Sticky at lg: so it stays beside whatever is being edited, and
           a page viewer with its own height so every page is reachable from it. -->
      <aside class="mt-6 lg:sticky lg:top-0 lg:mt-0 lg:self-start">
        <div
          class="card h-[min(80dvh,860px)] overflow-hidden lg:h-[calc(100dvh-8rem)] {view
            ? ''
            : 'flex items-center justify-center'}"
        >
          {#if view}
            <ResumePreview
              pages={view.pages}
              maxPages={resume.theme?.max_pages ?? 1}
              fill={view.fill}
              label="Your résumé"
            />
          {:else}
            <Loader size={20} class="animate-spin text-subtle" />
          {/if}
        </div>
        {#if view && view.pages.length > (resume.theme?.max_pages ?? 1)}
          <p class="mt-2 text-xs text-star">
            This runs to {view.pages.length} pages. Raise the page limit under Design, or trim a
            few bullets.
          </p>
        {/if}
        {#if !feed.needsSkillsSetup && doc.sections.some((s) => s.kind === "skills" && s.entries.length === 0)}
          <p class="mt-2 text-xs text-subtle">
            Your skills line comes from the {feed.skills.length} skills on your profile.
          </p>
        {/if}
      </aside>
    </div>
  {/if}
  </div>
  </div>
</div>
