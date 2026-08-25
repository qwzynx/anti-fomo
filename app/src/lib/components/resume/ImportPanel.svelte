<script lang="ts">
  import Field from "./Field.svelte";
  import { Check, Copy, Sparkles, Upload } from "$lib/icons";
  import { feed } from "$lib/feed.svelte";
  import { copyText } from "$lib/clipboard";
  import { buildImportPrompt } from "$lib/resumePrompt";

  /**
   * Getting a résumé that already exists somewhere else into this app.
   *
   * Two ways in, in the order people actually have their work history. The
   * first is a `resume.json` from another tool, which is what the paste box
   * has always been for. The second is the common case: the documents are a
   * pile of PDFs and notes in a chat thread, and what is missing is the
   * instruction that turns them into this format correctly. That instruction
   * is generated in `resumePrompt.ts` against the live skill catalog, so it
   * cannot drift from what the importer reads or what the matcher knows.
   *
   * The prompt stays visible in a textarea rather than living only on the
   * clipboard, because the clipboard is the step most likely to fail silently
   * on Android and a prompt you can select by hand always works.
   */
  let {
    text = $bindable(""),
    busy = false,
    hint = "",
    onImport,
  }: {
    text?: string;
    busy?: boolean;
    hint?: string;
    onImport: () => void;
  } = $props();

  const prompt = $derived(buildImportPrompt(feed.skillCatalog));

  let showPrompt = $state(false);
  let copied = $state(false);
  let copyFailed = $state(false);

  async function copyPrompt() {
    const ok = await copyText(prompt);
    copied = ok;
    copyFailed = !ok;
    // A copy that did not happen is not a step the user can skip, so the text
    // comes out where they can select it themselves.
    if (!ok) showPrompt = true;
    if (ok) setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="space-y-4">
  <div class="rounded-xl border border-line-soft bg-line-soft/40 p-4">
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div class="min-w-0">
        <p class="flex items-center gap-1.5 text-sm font-semibold">
          <Sparkles size={14} class="text-brand" />
          Don't have a resume.json?
        </p>
        <p class="mt-1 text-xs text-muted">
          Copy this prompt into a chat with Claude, attach the CVs, notes and project
          write-ups you already have there, and paste back the JSON it returns. It asks for
          eight to twelve bullets a role — one aimed at backend, one at security, one at
          scale, and so on — because that surplus is what each posting gets tailored from.
        </p>
      </div>
      <div class="flex shrink-0 gap-2">
        <button
          onclick={copyPrompt}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-3 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
        >
          {#if copied}<Check size={14} />Copied{:else}<Copy size={14} />Copy prompt{/if}
        </button>
        <button
          onclick={() => (showPrompt = !showPrompt)}
          aria-expanded={showPrompt}
          class="rounded-xl border border-line px-3 py-2 text-sm font-semibold hover:border-brand"
        >
          {showPrompt ? "Hide" : "View"}
        </button>
      </div>
    </div>

    {#if copyFailed}
      <p class="mt-2 text-xs text-danger" role="alert">
        This device would not let the app write to the clipboard. Select the text below and
        copy it by hand.
      </p>
    {/if}

    {#if showPrompt}
      <textarea
        readonly
        rows={12}
        value={prompt}
        onfocus={(e) => e.currentTarget.select()}
        class="control control-focus mt-3 block w-full resize-y font-mono text-xs leading-relaxed"
      ></textarea>
    {/if}
  </div>

  <div>
    <Field
      label="resume.json"
      bind:value={text}
      multiline
      rows={10}
      placeholder={'{ "basics": { "name": "…" }, "work": [ … ] }'}
      hint={hint || "The open jsonresume.org format. Paste the file's contents here."}
    />
    <button
      onclick={onImport}
      disabled={busy || !text.trim()}
      class="mt-3 inline-flex items-center gap-1.5 rounded-xl bg-brand px-4 py-2 text-sm font-semibold text-brand-fg disabled:opacity-50"
    >
      <Upload size={14} />
      Import
    </button>
  </div>
</div>
