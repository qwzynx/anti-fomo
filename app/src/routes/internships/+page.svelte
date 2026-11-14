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
  import * as api from "$lib/api";
  import {
    highlightTerms,
    indexableTerms,
    parseQuery,
    search,
    type DescriptionHits,
  } from "$lib/query";
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
   * What the full-text index says about the query's words, for the postings
   * whose description mentions one. Arrives a beat after the row matches; a
   * stale answer (the query moved on) is dropped by its key.
   */
  let described = $state.raw<{ key: string; hits: DescriptionHits } | null>(null);
  const describedKey = $derived(indexableTerms(parsed).join(" "));
  $effect(() => {
    const key = describedKey;
    if (!key) {
      described = null;
      return;
    }
    let live = true;
    api.searchDescriptions(key.split(" ")).then(
      (raw) => {
        if (!live) return;
        const hits: DescriptionHits = new Map();
        for (const [term, urls] of Object.entries(raw)) hits.set(term, new Set(urls));
        described = { key, hits };
      },
      () => {
        // The rows' own text still answers; the index is a bonus.
        if (live) described = null;
      },
    );
    return () => {
      live = false;
    };
  });
  const descriptionHits = $derived(described?.key === describedKey ? described.hits : undefined);

  /**
   * Search first, over the stable snapshot whose index is memoized on it, then
   * the chips, then the sort. Nothing here builds a string per row unless a
   * specialty chip asks for one.
   */
  const result = $derived.by(() => {
    // A plain copy for the pass: the filter reads a dozen fields per row, and
    // through the reactive proxy that is a dozen tracked reads times 40,000.
    const f = $state.snapshot(roles.filters);
    const hits = search(items, parsed, descriptionHits);
    const kept = filterRoles(hits.items, f, (i) => feed.match(i));
    const sorted = sortItems(kept, f.sort);
    return f.sort === "Best match" ? byQueryTier(sorted, hits.tier) : sorted;
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
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey) return;
    // A menu (sort, saved searches) owns its own arrow keys.
    if ((e.target as Element | null)?.closest?.('[role="menu"], [role="menuitemradio"]')) return;
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
  <div class="flex flex-col gap-1.5">
    <p class="eyebrow">{label}</p>
    <div class="flex flex-wrap gap-1">
      {#each options as option (option)}
        {@const active = current.includes(option)}
        <button
          onclick={() => onToggle(option)}
          aria-pressed={active}
          class="chip {active
            ? 'bg-brand text-brand-fg'
            : 'border border-line bg-surface text-muted hover:border-subtle/60 hover:text-foreground'}"
        >
          {option}
        </button>
      {/each}
    </div>
  </div>
{/snippet}

<!--
  A view in three bands: the toolbar (search, saved searches, sort), a strip of
  filter chips, and the results. At lg the results are two panes side by side
  — the list and the posting — each scrolling on its own, the way a mail client
  shows a message beside its inbox. Below lg the list takes the width and a
  posting opens as a sheet.
-->
<div class="flex h-full min-h-0 flex-col">
  <PageHeader title="Roles" subtitle={subtitle}>
    <label
      class="flex h-8 w-full max-w-md min-w-0 items-center gap-2 rounded-md border border-line bg-surface px-2.5 transition-colors focus-within:border-brand"
    >
      <Search size={14} class="shrink-0 text-subtle" />
      <input
        bind:this={searchInput}
        value={draft}
        oninput={(e) => onDraft(e.currentTarget.value)}
        type="search"
        placeholder="Search roles, companies, skills"
        aria-label="Search roles"
        title={'Every word must match. "exact phrase" · -exclude · company:name'}
        autocomplete="off"
        spellcheck="false"
        class="h-full min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-subtle [&::-webkit-search-cancel-button]:hidden"
      />
      {#if draft}
        <button
          onclick={clearQuery}
          aria-label="Clear search"
          class="-mr-1 flex h-6 w-6 shrink-0 items-center justify-center rounded text-subtle hover:bg-hover hover:text-foreground"
        >
          <X size={13} />
        </button>
      {:else}
        <span class="kbd hidden lg:inline-flex" title="Press / to search">/</span>
      {/if}
    </label>
    {#snippet actions()}
      <SavedSearches {items} />
      <SortMenu bind:value={f.sort} />
    {/snippet}
  </PageHeader>

  <!-- The filter strip. What is on is filled and carries a cross; nothing that
       is on ever hides in the Filters drawer. -->
  <div class="shrink-0 border-b border-line">
    <div class="scrollbar-hide flex items-center gap-1.5 overflow-x-auto px-4 py-2 lg:flex-wrap">
      <button
        onclick={() => (moreOpen = !moreOpen)}
        aria-expanded={moreOpen}
        aria-controls="roles-filters"
        class="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md border px-2.5 text-[12px] font-medium transition-colors
               {moreOpen || activeFilters > 0
          ? 'border-brand/40 bg-brand-soft text-brand-soft-fg'
          : 'border-line bg-surface text-muted hover:text-foreground'}"
      >
        <Filter size={12} strokeWidth={2.25} class="shrink-0" />
        Filters
        {#if activeFilters > 0}
          <span class="tabular-nums">{activeFilters}</span>
        {/if}
        <ChevronDown size={12} strokeWidth={2.25} class="shrink-0 transition-transform {moreOpen ? 'rotate-180' : ''}" />
      </button>
      <span class="mx-0.5 h-4 w-px shrink-0 bg-line" aria-hidden="true"></span>
      {#each quickChips as chip (chip.key)}
        <button
          onclick={chip.onToggle}
          aria-pressed={chip.active}
          class="inline-flex h-7 shrink-0 items-center gap-1 rounded-md px-2.5 text-[12px] transition-colors
                 {chip.active
            ? 'bg-brand font-semibold text-brand-fg'
            : 'border border-line bg-surface font-medium text-muted hover:border-subtle/60 hover:text-foreground'}"
        >
          {chip.label}
          {#if chip.active}
            <X size={11} strokeWidth={2.5} class="shrink-0 opacity-80" />
          {/if}
        </button>
      {/each}
      {#if narrowed}
        <button onclick={clearAll} class="ml-1 shrink-0 text-[12px] font-medium text-subtle hover:text-foreground">
          Clear all
        </button>
      {/if}
    </div>

    {#if moreOpen}
      <div
        id="roles-filters"
        class="pane animate-fade-in grid max-h-[45vh] gap-4 border-t border-line-soft bg-sidebar px-4 py-3 lg:grid-cols-3"
      >
        {@render facet("Type", TYPES.filter((t) => t !== "All"), f.type === "All" ? [] : [f.type], (v) => (f.type = f.type === v ? "All" : (v as TypeFilter)))}
        {@render facet("Level", levels, f.seniority, (v) => (f.seniority = toggle(f.seniority, v)))}
        {@render facet("Work mode", MODALITIES.filter((m) => m !== "All"), f.modality === "All" ? [] : [f.modality], (v) => (f.modality = f.modality === v ? "All" : (v as Modality)))}
        {@render facet("Specialty", SPECIALTIES, f.specialties, (v) => (f.specialties = toggle(f.specialties, v)))}
        {@render facet("Location", locations, f.locations, (v) => (f.locations = toggle(f.locations, v)))}
        {@render facet("Posted within", FRESHNESS.map((fr) => fr.label), [f.freshness], (v) => (f.freshness = v as FreshnessLabel))}
        {@render facet("Pay at least (per year)", PAY_FLOORS.map(formatK), f.minPay === null ? [] : [formatK(f.minPay)], (v) => {
          const n = PAY_FLOORS.find((p) => formatK(p) === v) ?? null;
          f.minPay = f.minPay === n ? null : n;
        })}
        {@render facet("Pay", ["States pay"], f.payDisclosed ? ["States pay"] : [], () => (f.payDisclosed = !f.payDisclosed))}
        {@render facet("Deadline", DEADLINES.filter((d) => d !== "Any"), f.deadline === "Any" ? [] : [f.deadline], (v) => (f.deadline = f.deadline === v ? "Any" : (v as DeadlineFilter)))}
        {@render facet("Discipline", HUB_DISCIPLINES, [f.discipline], (v) => (f.discipline = v))}
        <div class="lg:col-span-2">
          {@render facet("Source", allSources, f.sources, (v) => (f.sources = toggle(f.sources, v)))}
        </div>
        <p class="text-[12px] text-subtle lg:col-span-3">
          Level and pay only match postings that state them. Pay is compared per year, in the
          posting's own currency.
        </p>
      </div>
    {/if}
  </div>

  <div class="min-h-0 flex-1">
    {#if feed.loading}
      <div class="p-2"><RowSkeleton count={10} /></div>
    {:else if result.length > 0}
      <div class="flex h-full min-h-0">
        <div
          bind:this={listEl}
          class="pane w-full lg:w-[400px] lg:shrink-0 lg:border-r lg:border-line"
        >
          {#each visible as item (item.url)}
            <ItemRow
              {item}
              variant={wide ? "dense" : "band"}
              selected={selected?.url === item.url}
              onOpen={(i) => (selected = i)}
              {marks}
            />
          {/each}
          <InfiniteScroll {shown} total={result.length} noun="roles" onMore={() => (shown += PAGE)} />
        </div>

        {#if wide}
          <section class="flex min-w-0 flex-1 flex-col bg-background" aria-label="Posting">
            {#if selected}
              {#key selected.url}
                <ItemDetail item={selected} />
              {/key}
            {/if}
            <p class="shrink-0 border-t border-line px-4 py-1.5 text-[11px] text-subtle">
              <span class="kbd">j</span> <span class="kbd">k</span> move
              <span class="kbd ml-2">o</span> open
              <span class="kbd ml-2">s</span> save
              <span class="kbd ml-2">x</span> hide
              <span class="kbd ml-2">/</span> search
            </p>
          </section>
        {/if}
      </div>
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
  </div>
</div>

<!-- Only below lg: at desktop widths the pane above is already showing this. -->
{#if !wide}
  <ItemModal item={selected} onClose={() => (selected = null)} />
{/if}
