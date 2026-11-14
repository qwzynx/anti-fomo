<script lang="ts">
  import { tick } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { ScrapedItem } from "$lib/api";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import InfiniteScroll from "$lib/components/InfiniteScroll.svelte";
  import ItemDetail from "$lib/components/ItemDetail.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import RowSkeleton from "$lib/components/RowSkeleton.svelte";
  import SavedSearches from "$lib/components/SavedSearches.svelte";
  import SortMenu from "$lib/components/SortMenu.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Briefcase, ChevronDown, Filter, Search, X } from "$lib/icons";
  import {
    DEADLINES,
    FRESHNESS,
    HUB_DISCIPLINES,
    INTEREST_SPECIALTY,
    MODALITIES,
    PAY_FLOORS,
    SPECIALTIES,
    TYPES,
    activeFilterCount,
    byQueryTier,
    filterRoles,
    locationOptions,
    sortItems,
    toggle,
    type FreshnessLabel,
    type Modality,
    type TypeFilter,
  } from "$lib/filters";
  import { highlightTerms, parseQuery, search } from "$lib/query";
  import { roles, type DeadlineFilter } from "$lib/roles.svelte";

  // The job-board layout: a scrolling result list beside a detail pane that
  // stays put. Below lg there is no room for two columns, so the list goes full
  // width and the detail opens as a modal instead.
  //
  // One filter surface: a search field, then chips. What is on is filled and
  // carries a cross, what is off is an outline, and nothing that is on ever
  // hides in the "Filters" drawer. The state lives in `roles`, outside this
  // page, so it survives navigation and restarts; the ⌘K palette's "See all"
  // lands here with its query applied.

  const f = roles.filters;

  let selected = $state<ScrapedItem | null>(null);
  let moreOpen = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLElement | null>(null);

  /** What is typed, ahead of the debounced query the list is filtered by. */
  let draft = $state(roles.filters.query);
  let draftTimer: ReturnType<typeof setTimeout> | undefined;
  /** Short enough to feel live, long enough not to re-filter 40k rows per key. */
  const QUERY_DEBOUNCE_MS = 120;

  const PAGE = 40;
  let shown = $state(PAGE);

  /** True once there is room for two columns. Drives which of the detail pane
   *  and the modal is mounted — never both, or the modal's scroll lock would
   *  freeze the page behind a pane that is already visible. */
  let wide = $state(false);

  $effect(() => {
    void roles.init();
    const mq = window.matchMedia("(min-width: 1024px)");
    const sync = () => (wide = mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  });

  // Every change to the chips is written through (debounced) to settings.
  $effect(() => {
    void JSON.stringify(roles.filters);
    roles.changed();
  });

  // A query set from elsewhere — a saved search, the palette — shows in the
  // field. Not while the field has focus: the debounced write would otherwise
  // echo back over keys typed since.
  $effect(() => {
    const q = roles.filters.query;
    if (document.activeElement !== searchInput) draft = q;
  });

  function onDraft(value: string) {
    draft = value;
    clearTimeout(draftTimer);
    draftTimer = setTimeout(() => (roles.filters.query = draft), QUERY_DEBOUNCE_MS);
  }

  function clearQuery() {
    clearTimeout(draftTimer);
    draft = "";
    roles.filters.query = "";
  }

  const items = $derived(feed.internships);
  const allSources = $derived([...new Set(items.map((i) => i.source_platform))].sort());
  const locations = $derived(locationOptions(feed.setupOptions.regions));
  const levels = $derived(feed.setupOptions.seniority);

  const parsed = $derived(parseQuery(roles.filters.query));
  const marks = $derived(highlightTerms(parsed));

  /**
   * Search first, over the stable snapshot whose index is memoized on it, then
   * the chips, then the sort. Nothing here builds a string per row unless a
   * specialty chip asks for one.
   */
  const result = $derived.by(() => {
    const hits = search(items, parsed);
    const kept = filterRoles(hits.items, roles.filters, (i) => feed.match(i));
    const sorted = sortItems(kept, roles.filters.sort);
    return roles.filters.sort === "Best match" ? byQueryTier(sorted, hits.tier) : sorted;
  });

  // Back to the top when the filters change the list, but leave paging alone
  // when a background refresh lands on the same filter.
  const signature = $derived(`${result.length}|${result[0]?.url ?? ""}`);
  $effect(() => {
    void signature;
    shown = PAGE;
  });

  const visible = $derived(result.slice(0, shown));
  const resultUrls = $derived(new Set(result.map((i) => i.url)));

  /** At desktop widths the pane opens on the top result. Re-picks when a filter leaves the selection behind. */
  $effect(() => {
    if (!wide) return;
    if (!selected || !resultUrls.has(selected.url)) selected = result[0] ?? null;
  });

  // --- chips ---

  type Chip = { key: string; label: string; active: boolean; onToggle: () => void };

  /**
   * The chips on the bar itself: everything currently on, then a few worth
   * reaching for, chosen from the reader's own profile — their home region,
   * the specialties their interests name, the levels they said they want —
   * rather than one city and one stack for everybody.
   */
  const quickChips = $derived.by(() => {
    const chips: Chip[] = [];
    const p = feed.prefs;

    for (const t of TYPES) {
      if (t === "All") continue;
      // Only offer the kind the reader collects both of; with one, the chip filters nothing.
      if (!(p.internships && p.jobs) && f.type !== t) continue;
      chips.push({
        key: `type:${t}`,
        label: t === "Job" ? "Jobs" : "Internships",
        active: f.type === t,
        onToggle: () => (f.type = f.type === t ? "All" : t),
      });
    }

    for (const m of MODALITIES) {
      if (m === "All") continue;
      if (m === "Remote" || f.modality === m)
        chips.push({
          key: `mode:${m}`,
          label: m,
          active: f.modality === m,
          onToggle: () => (f.modality = f.modality === m ? "All" : m),
        });
    }

    const home = feed.homeRegion;
    for (const l of locations) {
      if (l === home || f.locations.includes(l))
        chips.push({
          key: `loc:${l}`,
          label: l,
          active: f.locations.includes(l),
          onToggle: () => (f.locations = toggle(f.locations, l)),
        });
    }

    const fromInterests = new Set(
      feed.interests.map((i) => INTEREST_SPECIALTY[i]).filter(Boolean).slice(0, 2),
    );
    for (const s of SPECIALTIES) {
      if (fromInterests.has(s) || f.specialties.includes(s))
        chips.push({
          key: `spec:${s}`,
          label: s,
          active: f.specialties.includes(s),
          onToggle: () => (f.specialties = toggle(f.specialties, s)),
        });
    }

    const wantedLevels = new Set(feed.targetSeniority);
    for (const l of levels) {
      if (wantedLevels.has(l) || f.seniority.includes(l))
        chips.push({
          key: `lvl:${l}`,
          label: l,
          active: f.seniority.includes(l),
          onToggle: () => (f.seniority = toggle(f.seniority, l)),
        });
    }

    const week = FRESHNESS[2].label;
    for (const fr of FRESHNESS) {
      if (fr.label === "Any time") continue;
      if (fr.label === week || f.freshness === fr.label)
        chips.push({
          key: `fresh:${fr.label}`,
          label: fr.label === week ? "Posted this week" : fr.label,
          active: f.freshness === fr.label,
          onToggle: () => (f.freshness = f.freshness === fr.label ? "Any time" : fr.label),
        });
    }

    if (f.deadline !== "Any")
      chips.push({
        key: "deadline",
        label: f.deadline,
        active: true,
        onToggle: () => (f.deadline = "Any"),
      });
    if (f.minPay !== null)
      chips.push({
        key: "pay",
        label: `${formatK(f.minPay)}+/yr`,
        active: true,
        onToggle: () => (f.minPay = null),
      });
    if (f.payDisclosed && f.minPay === null)
      chips.push({ key: "paid", label: "States pay", active: true, onToggle: () => (f.payDisclosed = false) });

    for (const s of f.sources)
      chips.push({ key: `src:${s}`, label: s, active: true, onToggle: () => (f.sources = toggle(f.sources, s)) });

    if (f.discipline !== "All")
      chips.push({ key: "disc", label: f.discipline, active: true, onToggle: () => (f.discipline = "All") });

    if (feed.skills.length > 0)
      chips.push({
        key: "match",
        label: "60%+ skill match",
        active: f.goodMatchOnly,
        onToggle: () => (f.goodMatchOnly = !f.goodMatchOnly),
      });

    // On before off, so what you have chosen reads left to right.
    return [...chips.filter((c) => c.active), ...chips.filter((c) => !c.active)];
  });

  function formatK(n: number): string {
    return `$${Math.round(n / 1000)}k`;
  }

  const activeFilters = $derived(activeFilterCount(roles.filters));
  const narrowed = $derived(activeFilters > 0 || !parsed.empty);

  function clearAll() {
    clearTimeout(draftTimer);
    draft = "";
    roles.clear();
  }

  const subtitle = $derived(
    feed.loading
      ? "Loading roles…"
      : `${result.length.toLocaleString()} of ${items.length.toLocaleString()} open roles${
          activeFilters > 0 ? ` · ${activeFilters} filter${activeFilters > 1 ? "s" : ""}` : ""
        }`,
  );

  // --- keyboard ---

  function typing(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
  }

  /** Moves the selection, paging the list along and keeping the row in view. */
  function move(by: number) {
    if (result.length === 0) return;
    const at = selected ? result.findIndex((i) => i.url === selected!.url) : -1;
    const next = Math.max(0, Math.min(result.length - 1, at + by));
    if (next >= shown) shown = next + PAGE;
    selected = result[next];
    void tick().then(() =>
      listEl?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: "nearest" }),
    );
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === "/" && !typing(e.target)) {
      e.preventDefault();
      searchInput?.focus();
      searchInput?.select();
      return;
    }
    if (typing(e.target)) {
      if (e.key === "Escape" && e.target === searchInput) {
        if (draft) clearQuery();
        else searchInput?.blur();
      } else if (e.key === "ArrowDown" && e.target === searchInput && wide) {
        e.preventDefault();
        searchInput?.blur();
        move(selected ? 0 : 1);
      }
      return;
    }
    // A dialog or menu is up: its keys are its own.
    if (document.querySelector('[aria-modal="true"]')) return;
    if (!wide) return;

    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "o":
      case "Enter":
        if (selected && (e.key === "o" || e.target === document.body)) {
          e.preventDefault();
          void openUrl(selected.url).catch(() => {});
        }
        break;
      case "s":
        if (selected) void feed.toggleSaved(selected);
        break;
      case "x":
        if (selected) {
          const gone = selected;
          move(1);
          void feed.dismiss(gone);
        }
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet facet(label: string, options: readonly string[], current: string[], onToggle: (v: string) => void)}
  <div class="flex flex-col gap-2">
    <p class="text-[11px] font-bold tracking-wider text-subtle uppercase">{label}</p>
    <div class="flex flex-wrap gap-1.5">
      {#each options as option (option)}
        {@const active = current.includes(option)}
        <button
          onclick={() => onToggle(option)}
          aria-pressed={active}
          class="chip {active
            ? 'bg-brand text-brand-fg'
            : 'border border-line bg-surface text-muted hover:border-subtle'}"
        >
          {option}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<!--
  Below lg this is an ordinary scrolling page. At lg it becomes a fixed-height
  app pane so the two columns can own the scrolling and the detail pane's Apply
  button always sits on screen rather than hanging off the bottom.
-->
<main
  class="mx-auto flex w-full max-w-7xl flex-1 flex-col px-4 pb-8 sm:px-6 lg:h-[calc(100dvh-58px)] lg:flex-none lg:overflow-hidden lg:px-8 lg:pb-5"
>
  <div class="shrink-0">
    <PageHeader title="Roles" {subtitle} />
  </div>

  <div class="shrink-0 pb-3">
    <!-- The search field leads the bar: the first thing anyone does on a job board. -->
    <div class="mb-2.5 flex items-center gap-2">
      <label
        class="flex h-10 min-w-0 flex-1 items-center gap-2.5 rounded-xl border border-line bg-surface px-3 transition-colors focus-within:border-brand"
      >
        <Search size={16} class="shrink-0 text-subtle" />
        <input
          bind:this={searchInput}
          value={draft}
          oninput={(e) => onDraft(e.currentTarget.value)}
          type="search"
          placeholder='Search title, company, skill — try "react toronto" or company:shopify'
          aria-label="Search roles"
          autocomplete="off"
          spellcheck="false"
          class="h-full min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-subtle [&::-webkit-search-cancel-button]:hidden"
        />
        {#if draft}
          <button
            onclick={clearQuery}
            aria-label="Clear search"
            class="-mr-1 flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-subtle hover:bg-line-soft hover:text-foreground"
          >
            <X size={14} />
          </button>
        {:else}
          <kbd
            class="hidden shrink-0 rounded-md border border-line px-1.5 py-0.5 text-[11px] font-semibold text-subtle lg:block"
            title="Press / to search">/</kbd
          >
        {/if}
      </label>
      <SavedSearches {items} />
      <SortMenu bind:value={f.sort} />
    </div>

    <div class="flex items-center gap-2">
      <div
        class="scrollbar-hide -mx-4 min-w-0 flex-1 overflow-x-auto px-4 sm:mx-0 sm:overflow-visible sm:px-0"
      >
        <div class="flex items-center gap-2 sm:flex-wrap">
          {#each quickChips as chip (chip.key)}
            <button
              onclick={chip.onToggle}
              aria-pressed={chip.active}
              class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3 text-[13px] transition-colors
                     {chip.active
                ? 'bg-brand font-semibold text-brand-fg'
                : 'border border-line bg-surface font-medium text-muted hover:border-subtle'}"
            >
              {chip.label}
              {#if chip.active}
                <X size={12} strokeWidth={2.5} class="shrink-0" />
              {/if}
            </button>
          {/each}

          <button
            onclick={() => (moreOpen = !moreOpen)}
            aria-expanded={moreOpen}
            aria-controls="roles-filters"
            class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full border px-3 text-[13px] font-semibold transition-colors
                   {moreOpen
              ? 'border-brand bg-brand-soft text-brand-soft-fg'
              : 'border-line bg-surface text-muted hover:border-subtle'}"
          >
            <Filter size={13} strokeWidth={2.25} class="shrink-0" />
            Filters
            {#if activeFilters > 0}
              <span
                class="rounded-full px-1.5 text-[11px] font-bold tabular-nums
                       {moreOpen ? 'bg-brand-soft-fg/15' : 'bg-line text-muted'}"
              >
                {activeFilters}
              </span>
            {/if}
            <ChevronDown
              size={13}
              strokeWidth={2.25}
              class="shrink-0 transition-transform {moreOpen ? 'rotate-180' : ''}"
            />
          </button>

          {#if narrowed}
            <button onclick={clearAll} class="shrink-0 text-[13px] font-semibold text-brand hover:underline">
              Clear all
            </button>
          {/if}
        </div>
      </div>
    </div>

    {#if moreOpen}
      <div
        id="roles-filters"
        class="animate-fade-in mt-3 grid max-h-[50vh] gap-4 overflow-y-auto rounded-xl border border-line bg-surface p-4 lg:grid-cols-2"
      >
        {@render facet("Type", TYPES.filter((t) => t !== "All"), f.type === "All" ? [] : [f.type], (v) => (f.type = f.type === v ? "All" : (v as TypeFilter)))}
        {@render facet("Level", levels, f.seniority, (v) => (f.seniority = toggle(f.seniority, v)))}
        {@render facet("Specialty", SPECIALTIES, f.specialties, (v) => (f.specialties = toggle(f.specialties, v)))}
        {@render facet("Work mode", MODALITIES.filter((m) => m !== "All"), f.modality === "All" ? [] : [f.modality], (v) => (f.modality = f.modality === v ? "All" : (v as Modality)))}
        {@render facet("Location", locations, f.locations, (v) => (f.locations = toggle(f.locations, v)))}
        {@render facet("Pay at least (per year)", PAY_FLOORS.map(formatK), f.minPay === null ? [] : [formatK(f.minPay)], (v) => {
          const n = PAY_FLOORS.find((p) => formatK(p) === v) ?? null;
          f.minPay = f.minPay === n ? null : n;
        })}
        {@render facet("Pay", ["States pay"], f.payDisclosed ? ["States pay"] : [], () => (f.payDisclosed = !f.payDisclosed))}
        {@render facet("Deadline", DEADLINES.filter((d) => d !== "Any"), f.deadline === "Any" ? [] : [f.deadline], (v) => (f.deadline = f.deadline === v ? "Any" : (v as DeadlineFilter)))}
        {@render facet("Posted within", FRESHNESS.map((fr) => fr.label), [f.freshness], (v) => (f.freshness = v as FreshnessLabel))}
        {@render facet("Discipline", HUB_DISCIPLINES, [f.discipline], (v) => (f.discipline = v))}
        <div class="lg:col-span-2">
          {@render facet("Source", allSources, f.sources, (v) => (f.sources = toggle(f.sources, v)))}
        </div>
        <p class="text-xs text-subtle lg:col-span-2">
          Level and pay only match postings that state them. Pay is compared per year, in the
          posting's own currency.
        </p>
      </div>
    {/if}
  </div>

  {#if feed.loading}
    <div class="card overflow-hidden p-2">
      <RowSkeleton count={8} />
    </div>
  {:else if result.length > 0}
    <!-- The two columns split the pane's height and scroll independently. -->
    <div
      class="lg:grid lg:min-h-0 lg:flex-1 lg:grid-cols-[minmax(0,25.25rem)_minmax(0,1fr)] lg:gap-5"
    >
      <div bind:this={listEl} class="card overflow-hidden lg:h-full lg:overflow-y-auto">
        <div>
          {#each visible as item (item.url)}
            <ItemRow
              {item}
              variant={wide ? "dense" : "band"}
              selected={selected?.url === item.url}
              onOpen={(i) => (selected = i)}
              {marks}
            />
          {/each}
        </div>
        <InfiniteScroll {shown} total={result.length} noun="roles" onMore={() => (shown += PAGE)} />
      </div>

      {#if wide && selected}
        <aside class="card flex h-full flex-col overflow-hidden bg-elevated">
          {#key selected.url}
            <ItemDetail item={selected} />
          {/key}
        </aside>
      {/if}
    </div>
    {#if wide}
      <p class="mt-2 hidden shrink-0 text-center text-[11px] text-subtle lg:block">
        <kbd class="font-sans font-semibold">j</kbd>/<kbd class="font-sans font-semibold">k</kbd> move ·
        <kbd class="font-sans font-semibold">o</kbd> open ·
        <kbd class="font-sans font-semibold">s</kbd> save ·
        <kbd class="font-sans font-semibold">x</kbd> hide ·
        <kbd class="font-sans font-semibold">/</kbd> search
      </p>
    {/if}
  {:else if items.length === 0}
    <EmptyState
      icon={Briefcase}
      title={feed.refreshing ? "Collecting roles…" : "No roles cached yet"}
      body={feed.refreshing
        ? "Roles appear here as each source finishes."
        : "Refresh to pull the latest openings from all sources."}
      actionLabel={feed.refreshing ? "Stop" : "Refresh now"}
      onAction={() => (feed.refreshing ? feed.stop() : feed.refresh(true))}
    />
  {:else}
    <EmptyState
      icon={Search}
      title="No roles match"
      body={!parsed.empty && activeFilters === 0
        ? `Nothing matches “${roles.filters.query.trim()}”. Try fewer words, or a shorter one — words match at their start, so “eng” finds “engineer”.`
        : "Nothing fits every filter at once. Try dropping the location, level or pay."}
      actionLabel="Clear search and filters"
      onAction={clearAll}
    />
  {/if}
</main>

<!-- Only below lg: at desktop widths the pane above is already showing this. -->
{#if !wide}
  <ItemModal item={selected} onClose={() => (selected = null)} />
{/if}
