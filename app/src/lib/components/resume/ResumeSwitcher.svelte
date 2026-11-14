<script lang="ts">
  import { timeAgo } from "$lib/api";
  import { confirm } from "$lib/confirm.svelte";
  import { Check, ChevronDown, FileText, Pencil, Plus, Trash2 } from "$lib/icons";
  import { resume } from "$lib/resume.svelte";
  import { toast } from "$lib/toast.svelte";

  // Which résumé the builder is editing, and the few things you do to the set:
  // open another, rename, make one the default (the one tailoring starts
  // from), start a new one, delete. There used to be no way to reach any row
  // but the default, which is why an import had to quietly take that over.
  let open = $state(false);
  let renaming = $state(false);
  let draft = $state("");
  let nameInput = $state<HTMLInputElement | null>(null);

  const current = $derived(resume.list.find((r) => r.id === resume.id) ?? null);

  function close() {
    open = false;
    renaming = false;
  }

  function startRename() {
    draft = resume.name;
    renaming = true;
    queueMicrotask(() => nameInput?.select());
  }

  function commitRename() {
    const next = draft.trim();
    if (next && next !== resume.name) resume.rename(next);
    renaming = false;
  }

  async function run(what: string, action: () => Promise<unknown>) {
    try {
      await action();
    } catch (e) {
      toast.error(what, e);
    }
  }

  async function remove(id: string, name: string) {
    const ok = await confirm({
      title: `Delete “${name}”?`,
      body: "The résumé and every tailored copy of it are deleted from this device. This cannot be undone — export a resume.json first if you might want it back.",
      confirmLabel: "Delete résumé",
      danger: true,
    });
    if (!ok) return;
    await run("delete the résumé", () => resume.remove(id));
    toast.info(`Deleted “${name}”`);
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && !renaming && close()} />

<div class="relative min-w-0">
  {#if renaming}
    <form
      class="flex items-center gap-1.5"
      onsubmit={(e) => {
        e.preventDefault();
        commitRename();
      }}
    >
      <input
        bind:this={nameInput}
        bind:value={draft}
        onblur={commitRename}
        onkeydown={(e) => e.key === "Escape" && (renaming = false)}
        aria-label="Résumé name"
        maxlength="80"
        class="control control-focus h-9 w-56 text-sm font-semibold"
      />
    </form>
  {:else}
    <button
      onclick={() => (open = !open)}
      aria-haspopup="true"
      aria-expanded={open}
      class="inline-flex h-9 max-w-[16rem] items-center gap-1.5 rounded-xl border border-line px-3 text-sm font-semibold transition-colors hover:border-subtle"
    >
      <FileText size={14} class="shrink-0 text-subtle" />
      <span class="truncate">{resume.name}</span>
      {#if current?.is_default}
        <span class="shrink-0 rounded bg-brand-soft px-1.5 text-[10px] font-bold text-brand-soft-fg uppercase">
          Default
        </span>
      {/if}
      <ChevronDown size={14} class="shrink-0 text-subtle" />
    </button>
  {/if}

  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={close}></div>
    <div
      class="animate-fade-up card absolute left-0 z-50 mt-2 w-80 overflow-hidden bg-elevated p-1.5 shadow-2xl"
    >
      <p class="px-2.5 pt-1.5 pb-1 text-[11px] font-bold tracking-wider text-subtle uppercase">
        Your résumés
      </p>
      {#each resume.list as r (r.id)}
        {@const active = r.id === resume.id}
        <div class="group flex items-center rounded-xl {active ? 'bg-brand-soft' : 'hover:bg-line-soft'}">
          <button
            onclick={() => {
              close();
              void run("open that résumé", () => resume.open(r.id));
            }}
            class="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-2 text-left"
          >
            <span class="min-w-0 flex-1">
              <span
                class="block truncate text-[13px] font-semibold {active ? 'text-brand-soft-fg' : ''}"
              >
                {r.name}
              </span>
              <span class="block text-[11px] text-subtle">
                {r.is_default ? "Default · " : ""}edited {timeAgo(r.updated_at)}
              </span>
            </span>
            {#if active}<Check size={14} strokeWidth={2.5} class="shrink-0 text-brand-soft-fg" />{/if}
          </button>
          <button
            onclick={() => void remove(r.id, r.name)}
            aria-label="Delete {r.name}"
            class="mr-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-subtle opacity-60 group-hover:opacity-100 hover:bg-danger-soft hover:text-danger"
          >
            <Trash2 size={13} />
          </button>
        </div>
      {/each}

      <div class="mt-1 flex flex-col border-t border-line-soft pt-1.5">
        <button
          onclick={() => {
            close();
            startRename();
          }}
          class="flex items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[13px] font-semibold hover:bg-line-soft"
        >
          <Pencil size={14} class="text-subtle" />
          Rename “{resume.name}”
        </button>
        {#if current && !current.is_default}
          <button
            onclick={() => {
              close();
              void run("make it the default", async () => {
                await resume.makeDefault(current.id);
                toast.success(`“${current.name}” is now the default`);
              });
            }}
            class="flex items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[13px] font-semibold hover:bg-line-soft"
          >
            <Check size={14} class="text-subtle" />
            Make this the default
          </button>
        {/if}
        <button
          onclick={() => {
            close();
            void run("start a new résumé", () => resume.create("New résumé"));
          }}
          class="flex items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[13px] font-semibold text-brand hover:bg-brand-soft"
        >
          <Plus size={14} strokeWidth={2.5} />
          New résumé
        </button>
      </div>
      <p class="px-2.5 pt-1 pb-1.5 text-[11px] text-subtle">
        Tailoring uses the résumé open here. The default is the one that opens at launch.
      </p>
    </div>
  {/if}
</div>
