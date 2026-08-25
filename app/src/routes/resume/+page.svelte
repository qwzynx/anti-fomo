<script lang="ts">
  import PageHeader from "$lib/components/PageHeader.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import Field from "$lib/components/resume/Field.svelte";
  import EntryEditor from "$lib/components/resume/EntryEditor.svelte";
  import ResumePreview from "$lib/components/resume/ResumePreview.svelte";
  import ThemePicker from "$lib/components/resume/ThemePicker.svelte";
  import {
    Download,
    FileText,
    Loader,
    Palette,
    Plus,
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
  let notice = $state<string | null>(null);
  let importing = $state(false);
  let importText = $state("");
  let busy = $state(false);

  $effect(() => {
    void resume.load();
  });

  const doc = $derived(resume.doc);
  const view = $derived(resume.view);
  /** The fill gauge answers the only question anyone editing a résumé has. */
  const pageLabel = $derived(
    view
      ? view.pages.length === 1
        ? `1 page · ${Math.round(view.fill * 100)}% full`
        : `${view.pages.length} pages`
      : "",
  );

  function touch() {
    resume.touch();
  }

  function addEntry(section: ResumeSection) {
    section.entries = [...section.entries, emptyEntry()];
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
    busy = true;
    // Flush any pending debounce first, or the file could be a beat behind the
    // preview the user is looking at.
    await resume.save();
    const outcome = await saveResumePdf({
      id: resume.id ?? undefined,
      theme: resume.theme ?? undefined,
      filename: view.filename,
      reveal: true,
    });
    notice = outcomeMessage(outcome);
    busy = false;
  }

  async function exportJson() {
    busy = true;
    await resume.save();
    notice = outcomeMessage(await saveResumeJson(resume.id ?? undefined, resume.name));
    busy = false;
  }

  async function runImport() {
    if (!importText.trim()) return;
    busy = true;
    try {
      await resume.importJson(importText);
      importing = false;
      importText = "";
      notice = "Imported.";
    } catch (e) {
      notice = e instanceof Error ? e.message : String(e);
    }
    busy = false;
  }
</script>

<svelte:head><title>Résumé · Anti-FOMO</title></svelte:head>

<main class="mx-auto w-full max-w-7xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <PageHeader
    title="Résumé"
    subtitle="Write it once. Every role you open can tailor a copy of it to what that posting asks for."
  >
    {#snippet actions()}
      {#if resume.exists}
        <button
          onclick={downloadPdf}
          disabled={busy || !view}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-3.5 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
        >
          {#if busy}<Loader size={15} class="animate-spin" />{:else}<Download size={15} />{/if}
          Save PDF
        </button>
      {/if}
    {/snippet}
  </PageHeader>

  {#if resume.error}
    <p class="mt-3 rounded-xl bg-danger-soft p-3 text-sm text-danger" role="alert">
      {resume.error}
    </p>
  {/if}
  {#if notice}
    <p class="mt-3 rounded-xl bg-line-soft p-3 text-sm text-muted" role="status">{notice}</p>
  {/if}

  {#if resume.loading}
    <div class="mt-10 flex justify-center"><Loader size={22} class="animate-spin text-subtle" /></div>
  {:else if !resume.exists}
    <div class="mt-4">
      <EmptyState
        icon={FileText}
        title="No résumé yet"
        body="Start an empty one and fill in your experience, or paste a resume.json you already have."
        actionLabel="Start a résumé"
        onAction={() => void resume.create()}
        {busy}
      />
      <div class="mt-3 text-center">
        <button
          onclick={() => (importing = !importing)}
          class="inline-flex items-center gap-1.5 text-sm font-semibold text-brand hover:underline"
        >
          <Upload size={14} />
          Import a JSON Resume instead
        </button>
      </div>
      {#if importing}
        <div class="card mx-auto mt-4 max-w-2xl p-5">
          <Field
            label="resume.json"
            bind:value={importText}
            multiline
            rows={10}
            placeholder={'{ "basics": { "name": "…" }, "work": [ … ] }'}
            hint="The open jsonresume.org format. Paste the file's contents here."
          />
          <button
            onclick={runImport}
            disabled={busy || !importText.trim()}
            class="mt-3 rounded-xl bg-brand px-4 py-2 text-sm font-semibold text-brand-fg disabled:opacity-50"
          >
            Import
          </button>
        </div>
      {/if}
    </div>
  {:else if doc}
    <div class="mt-4 lg:grid lg:grid-cols-[minmax(0,1fr)_minmax(0,26rem)] lg:gap-6">
      <!-- Editor -->
      <div class="min-w-0">
        <div class="mb-4 flex flex-wrap items-center gap-2">
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

        {#if tab === "design"}
          <section class="card p-5">
            {#if resume.theme}
              <ThemePicker
                theme={resume.theme}
                options={resume.themes}
                onChange={(next) => resume.setTheme(next)}
              />
            {/if}
            <div class="mt-6 border-t border-line-soft pt-4">
              <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">
                Backup & transfer
              </p>
              <div class="flex flex-wrap gap-2">
                <button
                  onclick={exportJson}
                  disabled={busy}
                  class="inline-flex items-center gap-1.5 rounded-xl border border-line px-3 py-2 text-sm font-semibold hover:border-brand disabled:opacity-50"
                >
                  <Download size={14} />
                  Export resume.json
                </button>
                <button
                  onclick={() => (importing = !importing)}
                  class="inline-flex items-center gap-1.5 rounded-xl border border-line px-3 py-2 text-sm font-semibold hover:border-brand"
                >
                  <Upload size={14} />
                  Import
                </button>
              </div>
              <p class="mt-2 text-xs text-subtle">
                Everything is on this device only. An export is the one copy that is not.
              </p>
              {#if importing}
                <div class="mt-3">
                  <Field
                    label="resume.json"
                    bind:value={importText}
                    multiline
                    rows={8}
                    hint="Imports as a new résumé; the current one is left alone."
                  />
                  <button
                    onclick={runImport}
                    disabled={busy || !importText.trim()}
                    class="mt-2 rounded-xl bg-brand px-4 py-2 text-sm font-semibold text-brand-fg disabled:opacity-50"
                  >
                    Import
                  </button>
                </div>
              {/if}
            </div>
          </section>
        {:else}
          <section class="card mb-5 p-5">
            <h3 class="mb-3 text-lg font-bold">You</h3>
            <div class="grid gap-3 sm:grid-cols-2">
              <Field
                label="Full name"
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
                label="Email"
                bind:value={doc.contact.email}
                oninput={touch}
                placeholder="you@example.com"
              />
              <Field
                label="Phone"
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
                    label="URL"
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
            <section class="card mb-5 p-5">
              <div class="mb-3 flex items-baseline justify-between gap-3">
                <input
                  bind:value={section.title}
                  oninput={touch}
                  aria-label="Section name"
                  class="min-w-0 flex-1 border-0 bg-transparent p-0 text-lg font-bold focus:outline-none"
                />
                <span class="shrink-0 text-xs text-subtle">
                  {section.entries.length}
                  {section.entries.length === 1 ? "entry" : "entries"}
                </span>
              </div>

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
            </section>
          {/each}
        {/if}
      </div>

      <!-- Preview. Sticky at lg: so it stays beside whatever is being edited. -->
      <aside class="mt-6 lg:sticky lg:top-[70px] lg:mt-0 lg:self-start">
        <div class="mb-2 flex items-baseline justify-between gap-2">
          <p class="text-xs font-semibold tracking-wide text-subtle uppercase">Preview</p>
          <span class="text-xs text-subtle">{pageLabel}</span>
        </div>
        {#if view}
          <div class="max-h-[calc(100dvh-140px)] overflow-y-auto rounded-xl bg-line-soft p-3">
            <ResumePreview pages={view.pages} scale={0.62} label="Your résumé" />
          </div>
          {#if view.pages.length > (resume.theme?.max_pages ?? 1)}
            <p class="mt-2 text-xs text-star">
              This runs to {view.pages.length} pages. Raise the page limit under Design, or trim a
              few bullets.
            </p>
          {/if}
        {:else}
          <div class="flex h-64 items-center justify-center rounded-xl bg-line-soft">
            <Loader size={20} class="animate-spin text-subtle" />
          </div>
        {/if}
        {#if !feed.needsSkillsSetup && doc.sections.some((s) => s.kind === "skills" && s.entries.length === 0)}
          <p class="mt-2 text-xs text-subtle">
            Your skills line comes from the {feed.skills.length} skills on your profile.
          </p>
        {/if}
      </aside>
    </div>
  {/if}
</main>
