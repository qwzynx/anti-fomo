<script lang="ts">
  import { goto } from "$app/navigation";
  import * as api from "$lib/api";
  import type { PastedPosting, ScrapedItem } from "$lib/api";
  import { confirm } from "$lib/confirm.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, ClipboardPaste, FileText, ListChecks, Loader, Search, Trash2 } from "$lib/icons";
  import { parseQuery, search } from "$lib/query";
  import { resume } from "$lib/resume.svelte";
  import { describeError, toast } from "$lib/toast.svelte";

  /**
   * Where tailoring starts when no posting was handed in — the sidebar's
   * "Tailor résumé".
   *
   * First, no job at all: a version chosen by hand — a general one-pager, a
   * version for a kind of role — stored as a `pasted:` posting with no text,
   * so it tailors against nothing and the reader's ticks decide the page.
   *
   * Then two ways to name a job. Paste its description, for a posting found
   * anywhere: it is stored on this device as a `pasted:` posting and read the
   * way a fetched one is. Or pick one the app already has — saved roles first,
   * since those are the ones being applied to, then anything in Roles by
   * search.
   */

  const RESULTS = 25;

  let title = $state("");
  let company = $state("");
  let text = $state("");
  let saving = $state(false);
  let versionName = $state("");
  let creating = $state(false);
  let query = $state("");
  let pasted = $state.raw<PastedPosting[]>([]);

  $effect(() => {
    void resume.ensure();
    api
      .listPastedPostings()
      .then((list) => (pasted = list))
      .catch((e) => toast.error("load your pasted jobs", e));
  });

  const isRole = (i: ScrapedItem) => i.item_type === "Internship" || i.item_type === "Job";
  const savedRoles = $derived(feed.saved.filter(isRole));

  const parsed = $derived(parseQuery(query));
  const matches = $derived.by(() => {
    if (parsed.empty) return { saved: savedRoles, roles: [] as readonly ScrapedItem[], more: 0 };
    const saved = search(savedRoles, parsed).items;
    const savedUrls = new Set(saved.map((i) => i.url));
    const all = search(feed.internships, parsed).items.filter((i) => !savedUrls.has(i.url));
    return { saved, roles: all.slice(0, RESULTS), more: Math.max(0, all.length - RESULTS) };
  });
  const pastedAll = $derived(
    parsed.empty
      ? pasted
      : pasted.filter((p) =>
          `${p.title} ${p.company ?? ""}`.toLowerCase().includes(query.trim().toLowerCase()),
        ),
  );

  const versions = $derived(pastedAll.filter((p) => p.by_hand));
  const pastedShown = $derived(pastedAll.filter((p) => !p.by_hand));

  function open(url: string) {
    void goto(`/resume/tailor?url=${encodeURIComponent(url)}`);
  }

  async function tailorPasted() {
    if (!text.trim() || saving) return;
    saving = true;
    try {
      const url = await api.savePastedPosting(title, company.trim() || null, text);
      open(url);
    } catch (e) {
      toast.error("save that job description", describeError(e));
    } finally {
      saving = false;
    }
  }

  /** A version with no job behind it: the reader picks what goes on the page. */
  async function startByHand() {
    if (creating) return;
    creating = true;
    try {
      open(await api.savePastedPosting(versionName, null, ""));
    } catch (e) {
      toast.error("start that version", describeError(e));
    } finally {
      creating = false;
    }
  }

  async function removePasted(p: PastedPosting) {
    const ok = await confirm({
      title: `Delete “${p.title}”?`,
      body: p.by_hand
        ? "This version and the choices you made in it are removed. Your résumé is not touched."
        : "The pasted description and the tailoring you made for it are both removed. Your résumé is not touched.",
      confirmLabel: "Delete",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deletePastedPosting(p.url);
      pasted = pasted.filter((x) => x.url !== p.url);
    } catch (e) {
      toast.error("delete that job", e);
    }
  }

  const when = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
</script>

{#snippet row(url: string, heading: string, sub: string, icon: "saved" | "pasted" | "role" | "hand")}
  <button
    onclick={() => open(url)}
    class="flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors hover:bg-hover"
  >
    <span class="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-line-soft text-subtle">
      {#if icon === "hand"}<ListChecks size={13} />{:else if icon === "saved"}<Bookmark size={13} />{:else if icon === "pasted"}<ClipboardPaste
          size={13}
        />{:else}<FileText size={13} />{/if}
    </span>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[13px] font-medium">{heading}</span>
      {#if sub}<span class="block truncate text-[11.5px] text-subtle">{sub}</span>{/if}
    </span>
  </button>
{/snippet}

<div class="pane flex-1">
  <div class="mx-auto grid w-full max-w-6xl gap-5 px-4 py-5 sm:px-6 lg:grid-cols-2">
    {#if !resume.loading && !resume.exists}
      <p class="rounded-md border border-star/30 bg-star-soft/50 px-3 py-2 text-[12.5px] text-muted lg:col-span-2">
        You have no résumé yet, so there is nothing to tailor.
        <a href="/resume" class="font-semibold text-brand hover:underline">Build one first</a>.
      </p>
    {/if}

    <!-- No job -->
    <section class="card flex flex-col gap-3 p-4 sm:flex-row sm:items-center lg:col-span-2">
      <div class="min-w-0 flex-1">
        <h2 class="flex items-center gap-2 text-[14px] font-semibold">
          <ListChecks size={15} class="text-brand" />
          No job in mind?
        </h2>
        <p class="mt-1 text-[12.5px] text-muted">
          Make a version by hand — a general one-pager, or one for a kind of role. Pick the
          sections, entries and bullets yourself; your résumé itself is not changed.
        </p>
      </div>
      <div class="flex shrink-0 gap-2">
        <input
          bind:value={versionName}
          placeholder="General résumé"
          maxlength="120"
          aria-label="Name of this version"
          onkeydown={(e) => {
            if (e.key === "Enter") void startByHand();
          }}
          class="control control-focus w-full text-[13px] sm:w-56"
        />
        <button
          onclick={startByHand}
          disabled={creating || !resume.exists}
          class="btn-primary h-[34px] shrink-0 disabled:opacity-50"
        >
          {#if creating}<Loader size={14} class="animate-spin" />{/if}
          Choose by hand
        </button>
      </div>
    </section>

    <!-- Paste -->
    <section class="card flex flex-col p-4">
      <h2 class="flex items-center gap-2 text-[14px] font-semibold">
        <ClipboardPaste size={15} class="text-brand" />
        Paste a job description
      </h2>
      <p class="mt-1 text-[12.5px] text-muted">
        For a job you found anywhere. Paste the whole posting — its requirements and
        responsibilities are weighed like any listing's. It stays on this device.
      </p>
      <div class="mt-4 grid gap-3 sm:grid-cols-2">
        <label class="block">
          <span class="eyebrow mb-1 block">Job title</span>
          <input bind:value={title} placeholder="Backend Engineer Intern" maxlength="160" class="control control-focus w-full text-[13px]" />
        </label>
        <label class="block">
          <span class="eyebrow mb-1 block">Company</span>
          <input bind:value={company} placeholder="Optional" maxlength="120" class="control control-focus w-full text-[13px]" />
        </label>
      </div>
      <label class="mt-3 flex min-h-0 flex-1 flex-col">
        <span class="eyebrow mb-1 block">Description</span>
        <textarea
          bind:value={text}
          rows="14"
          placeholder="About the role… Responsibilities… Requirements…"
          onkeydown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void tailorPasted();
          }}
          class="control control-focus block min-h-48 w-full flex-1 resize-y text-[13px] leading-relaxed"
        ></textarea>
      </label>
      <div class="mt-3 flex items-center gap-3">
        <button
          onclick={tailorPasted}
          disabled={!text.trim() || saving || !resume.exists}
          class="btn-primary disabled:opacity-50"
        >
          {#if saving}<Loader size={14} class="animate-spin" />{/if}
          Tailor my résumé
        </button>
        <span class="text-[11.5px] text-subtle">
          <span class="kbd">Ctrl</span> <span class="kbd">Enter</span>
        </span>
      </div>
    </section>

    <!-- Pick -->
    <section class="card flex min-h-[24rem] flex-col overflow-hidden">
      <div class="border-b border-line p-4 pb-3">
        <h2 class="flex items-center gap-2 text-[14px] font-semibold">
          <Search size={15} class="text-brand" />
          Or open a job or a version
        </h2>
        <div class="relative mt-3">
          <Search size={14} class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-subtle" />
          <input
            bind:value={query}
            type="search"
            placeholder="Search saved and all roles — title, company:stripe"
            class="control control-focus w-full pl-8 text-[13px]"
          />
        </div>
      </div>

      <div class="pane flex-1 py-1">
        {#each [{ label: "Your versions", list: versions }, { label: "Pasted before", list: pastedShown }] as group (group.label)}
          {#if group.list.length > 0}
          <p class="eyebrow px-3 pt-2 pb-1">{group.label}</p>
          {#each group.list as p (p.url)}
            <div class="group flex items-center">
              <div class="min-w-0 flex-1">
                {@render row(
                  p.url,
                  p.title,
                  [p.company, `${p.by_hand ? "made" : "pasted"} ${when.format(new Date(p.created_at))}`]
                    .filter(Boolean)
                    .join(" · "),
                  p.by_hand ? "hand" : "pasted",
                )}
              </div>
              <button
                onclick={() => removePasted(p)}
                aria-label="Delete {p.title}"
                title={p.by_hand ? "Delete this version" : "Delete this pasted job"}
                class="tool reveal-on-hover mr-2 h-7 min-w-7 px-0 hover:text-danger"
              >
                <Trash2 size={13} />
              </button>
            </div>
          {/each}
          {/if}
        {/each}

        {#if matches.saved.length > 0}
          <p class="eyebrow px-3 pt-3 pb-1">Saved roles</p>
          {#each matches.saved as i (i.url)}
            {@render row(i.url, i.title, [i.company, i.location].filter(Boolean).join(" · "), "saved")}
          {/each}
        {/if}

        {#if matches.roles.length > 0}
          <p class="eyebrow px-3 pt-3 pb-1">Roles</p>
          {#each matches.roles as i (i.url)}
            {@render row(i.url, i.title, [i.company, i.location].filter(Boolean).join(" · "), "role")}
          {/each}
          {#if matches.more > 0}
            <p class="px-3 py-2 text-[11.5px] text-subtle">
              {matches.more.toLocaleString()} more — narrow the search to see them.
            </p>
          {/if}
        {/if}

        {#if pastedAll.length === 0 && matches.saved.length === 0 && matches.roles.length === 0}
          <p class="px-4 py-10 text-center text-[12.5px] text-subtle">
            {#if parsed.empty}
              Save a role, or search every role the app has found.
            {:else}
              No role matches “{query.trim()}”.
            {/if}
          </p>
        {/if}
      </div>
    </section>
  </div>
</div>
