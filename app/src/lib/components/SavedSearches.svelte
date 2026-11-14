<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { filterRoles } from "$lib/filters";
  import { Check, History, Plus, Trash2 } from "$lib/icons";
  import { parseQuery, search } from "$lib/query";
  import { roles, type SavedSearch } from "$lib/roles.svelte";
  import { toast } from "$lib/toast.svelte";

  // Saved searches for Roles: a name for a query plus its chips, and how many
  // roles have turned up for it since it was last opened. The counts run the
  // whole filter pass once per saved search, so they are only computed while
  // the menu is open — never on every tap of every chip.
  let { items }: { items: readonly ScrapedItem[] } = $props();

  let open = $state(false);
  let naming = $state(false);
  let name = $state("");
  let nameInput = $state<HTMLInputElement | null>(null);

  const current = $derived(roles.ready ? roles.current() : null);

  /** Roles matching each saved search that were posted after it was last opened. */
  const fresh = $derived.by(() => {
    const counts = new Map<string, number>();
    if (!open) return counts;
    for (const s of roles.saved) {
      const since = s.viewedAt;
      const matched = filterRoles(
        search(items, parseQuery(s.filters.query)).items,
        s.filters,
        (i) => feed.match(i),
      );
      let n = 0;
      for (const item of matched) if (item.timestamp > since) n++;
      counts.set(s.id, n);
    }
    return counts;
  });

  function suggestName(): string {
    const f = roles.filters;
    const bits = [
      f.query.trim(),
      ...f.specialties,
      f.modality !== "All" ? f.modality : "",
      ...f.locations,
      f.type !== "All" ? (f.type === "Job" ? "Jobs" : "Internships") : "",
    ].filter(Boolean);
    return bits.slice(0, 3).join(" · ") || "All roles";
  }

  function startNaming() {
    name = suggestName();
    naming = true;
    queueMicrotask(() => nameInput?.select());
  }

  async function saveNow() {
    try {
      const entry = await roles.saveSearch(name);
      toast.success(`Saved “${entry.name}”`);
      naming = false;
      open = false;
    } catch (e) {
      toast.error("save this search", e);
    }
  }

  async function apply(s: SavedSearch) {
    open = false;
    await roles.apply(s.id);
  }

  async function remove(s: SavedSearch) {
    try {
      await roles.removeSearch(s.id);
      toast.info(`Removed “${s.name}”`);
    } catch (e) {
      toast.error("remove the saved search", e);
    }
  }

  function close() {
    open = false;
    naming = false;
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && close()} />

<div class="relative shrink-0">
  <button
    onclick={() => (open ? close() : (open = true))}
    aria-haspopup="true"
    aria-expanded={open}
    title="Saved searches"
    class="tool tool-outline shrink-0 {open || current ? 'text-foreground' : ''}"
  >
    <History size={13} strokeWidth={2.25} class="shrink-0" />
    <span class="hidden max-w-[10rem] truncate xl:inline">{current ? current.name : "Saved searches"}</span>
  </button>

  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={close}></div>

    <div
      class="animate-fade-up absolute right-0 z-50 mt-1.5 w-80 origin-top-right overflow-hidden rounded-lg border border-line bg-elevated p-1 shadow-xl shadow-black/10"
    >
      {#if roles.saved.length === 0 && !naming}
        <p class="px-3 pt-2.5 pb-1 text-[13px] text-muted">
          Save what you've searched and chosen here, and come back to it in one click — with a count
          of what's new since you last looked.
        </p>
      {/if}

      {#each roles.saved as s (s.id)}
        {@const count = fresh.get(s.id) ?? 0}
        {@const active = current?.id === s.id}
        <div class="group flex items-center rounded-xl {active ? 'bg-brand-soft' : 'hover:bg-line-soft'}">
          <button
            onclick={() => apply(s)}
            class="flex min-w-0 flex-1 items-center gap-2 px-2.5 py-2 text-left"
          >
            <span
              class="min-w-0 flex-1 truncate text-[13px] font-semibold {active
                ? 'text-brand-soft-fg'
                : 'text-foreground'}"
            >
              {s.name}
            </span>
            {#if count > 0}
              <span
                class="shrink-0 rounded-full bg-brand px-1.5 text-[11px] leading-5 font-bold text-brand-fg tabular-nums"
                title="{count} posted since you last opened this search"
              >
                {count > 99 ? "99+" : count} new
              </span>
            {/if}
            {#if active}
              <Check size={14} strokeWidth={2.5} class="shrink-0 text-brand-soft-fg" />
            {/if}
          </button>
          <button
            onclick={() => remove(s)}
            aria-label="Remove saved search {s.name}"
            class="mr-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-subtle opacity-60 hover:bg-danger-soft hover:text-danger group-hover:opacity-100"
          >
            <Trash2 size={13} />
          </button>
        </div>
      {/each}

      <div class="mt-1 border-t border-line-soft pt-1.5">
        {#if naming}
          <form
            class="flex items-center gap-1.5 px-1"
            onsubmit={(e) => {
              e.preventDefault();
              void saveNow();
            }}
          >
            <input
              bind:this={nameInput}
              bind:value={name}
              aria-label="Name for this search"
              maxlength="60"
              class="h-9 min-w-0 flex-1 rounded-lg border border-line bg-surface px-2.5 text-[13px] outline-none focus:border-brand"
            />
            <button
              type="submit"
              class="h-9 shrink-0 rounded-lg bg-brand px-3 text-[13px] font-semibold text-brand-fg hover:bg-brand-hover"
            >
              Save
            </button>
          </form>
        {:else}
          <button
            onclick={startNaming}
            disabled={current !== null}
            class="flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left text-[13px] font-semibold text-brand hover:bg-brand-soft disabled:cursor-default disabled:text-subtle disabled:hover:bg-transparent"
          >
            <Plus size={14} strokeWidth={2.5} />
            {current ? "This search is saved" : "Save this search"}
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>
