<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import type { ApplicationStatus, ScrapedItem } from "$lib/api";
  import { APPLICATION_STAGES, stageBadgeClass, stageLabel } from "$lib/applications";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import InfiniteScroll from "$lib/components/InfiniteScroll.svelte";
  import ItemDetail from "$lib/components/ItemDetail.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import RowSkeleton from "$lib/components/RowSkeleton.svelte";
  import { feed } from "$lib/feed.svelte";
  import { Bookmark, ListChecks } from "$lib/icons";
  import { eventDate, monthDay } from "$lib/item";

  // What you kept, as a mailbox: a list beside the item, filtered by kind and,
  // for roles, by where your application stands. The sidebar's "Applications"
  // is this same view at `?stage=active`, so the tracker is not a second place
  // to keep in step with the first.

  type Kind = "All" | "Roles" | "Reading" | "Events";
  /** "any", "none" (saved, not applied), "active" (applied → offer) or one stage. */
  type Stage = "any" | "none" | "active" | ApplicationStatus;

  const KINDS: { key: Kind; types: ScrapedItem["item_type"][] }[] = [
    { key: "All", types: ["Internship", "Job", "Article", "Event"] },
    { key: "Roles", types: ["Internship", "Job"] },
    { key: "Reading", types: ["Article"] },
    { key: "Events", types: ["Event"] },
  ];

  const PAGE = 40;

  let selected = $state<ScrapedItem | null>(null);
  let kind = $state<Kind>("All");
  let shown = $state(PAGE);
  let wide = $state(false);

  $effect(() => {
    const mq = window.matchMedia("(min-width: 1024px)");
    const sync = () => (wide = mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  });

  /** The stage lives in the URL, so the sidebar can link straight to it. */
  const stage = $derived((page.url.searchParams.get("stage") as Stage | null) ?? "any");
  const tracking = $derived(stage === "active");

  function setStage(next: Stage) {
    const url = new URL(page.url);
    if (next === "any") url.searchParams.delete("stage");
    else url.searchParams.set("stage", next);
    void goto(url, { replaceState: true, keepFocus: true, noScroll: true });
  }

  const isRole = (i: ScrapedItem) => i.item_type === "Internship" || i.item_type === "Job";
  const stageOf = (i: ScrapedItem) => feed.application(i.url)?.status ?? null;

  function stageMatches(i: ScrapedItem): boolean {
    if (stage === "any") return true;
    if (!isRole(i)) return false;
    const s = stageOf(i);
    if (stage === "none") return s === null;
    if (stage === "active") return s === "applied" || s === "interviewing" || s === "offer";
    return s === stage;
  }

  const kindTypes = $derived(
    new Set(KINDS.find((k) => k.key === (tracking ? "Roles" : kind))?.types ?? []),
  );
  const list = $derived(feed.saved.filter((i) => kindTypes.has(i.item_type) && stageMatches(i)));
  const visible = $derived(list.slice(0, shown));

  const kindCounts = $derived(
    new Map(KINDS.map((k) => [k.key, feed.saved.filter((i) => k.types.includes(i.item_type)).length])),
  );

  /** Counts per stage over the saved roles, for the stage chips. */
  const stageCounts = $derived.by(() => {
    const counts = new Map<string, number>([["none", 0]]);
    for (const i of feed.saved) {
      if (!isRole(i)) continue;
      const s = stageOf(i) ?? "none";
      counts.set(s, (counts.get(s) ?? 0) + 1);
    }
    return counts;
  });

  // Back to the top when the filter changes.
  $effect(() => {
    void kind;
    void stage;
    shown = PAGE;
  });

  const urls = $derived(new Set(list.map((i) => i.url)));
  $effect(() => {
    if (!wide) return;
    if (!selected || !urls.has(selected.url)) selected = list[0] ?? null;
  });

  /**
   * The row's trailing text: a role's stage and when it moved, an event's
   * countdown; otherwise the row's own default.
   */
  function metaFor(item: ScrapedItem): string | undefined {
    if (isRole(item)) {
      const app = feed.application(item.url);
      if (!app?.status) return undefined;
      const d = app.updated_at ? monthDay(app.updated_at) : null;
      return `${stageLabel(app.status)}${d ? ` · ${d.month} ${Number(d.day)}` : ""}`;
    }
    if (item.item_type !== "Event") return undefined;
    const days = Math.round((new Date(eventDate(item)).getTime() - Date.now()) / 86400000);
    const when = days === 0 ? "today" : days === 1 ? "tomorrow" : `in ${days} days`;
    if (days < 0) return "Already passed";
    if (item.closes_at) return `Closes ${when}`;
    return when.charAt(0).toUpperCase() + when.slice(1);
  }

  const subtitle = $derived(
    feed.loading
      ? "Loading…"
      : tracking
        ? `${list.length} in play — applied, interviewing or with an offer`
        : `${feed.saved.length} kept on this device, even after they leave the feed`,
  );

  const stageChips = $derived([
    { key: "any" as Stage, label: "Any stage", count: null as number | null },
    { key: "none" as Stage, label: "Not applied", count: stageCounts.get("none") ?? 0 },
    { key: "active" as Stage, label: "In play", count: null as number | null },
    ...APPLICATION_STAGES.map((s) => ({
      key: s.value as Stage,
      label: s.label,
      count: stageCounts.get(s.value) ?? 0,
    })),
  ]);

  const isStage = (k: Stage): k is ApplicationStatus => k !== "any" && k !== "none" && k !== "active";
</script>

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title={tracking ? "Applications" : "Saved"} {subtitle}>
    {#snippet actions()}
      {#if !tracking}
        <!-- A segmented control, the way an app picks a mailbox. -->
        <div
          class="hidden items-center rounded-md border border-line bg-surface p-0.5 sm:flex"
          role="group"
          aria-label="Kind"
        >
          {#each KINDS as k (k.key)}
            {@const n = kindCounts.get(k.key) ?? 0}
            {#if k.key === "All" || n > 0}
              <button
                onclick={() => (kind = k.key)}
                aria-pressed={kind === k.key}
                class="h-6 rounded px-2.5 text-[12px] font-medium transition-colors
                       {kind === k.key ? 'bg-hover text-foreground' : 'text-subtle hover:text-foreground'}"
              >
                {k.key}
                <span class="ml-0.5 tabular-nums opacity-70">{n}</span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    {/snippet}
  </PageHeader>

  {#if (kind === "All" || kind === "Roles" || tracking) && (stageCounts.size > 1 || stage !== "any")}
    <div class="shrink-0 border-b border-line">
      <div
        class="scrollbar-hide flex items-center gap-1.5 overflow-x-auto px-4 py-2"
        role="group"
        aria-label="Application stage"
      >
        <ListChecks size={13} class="mr-1 shrink-0 text-subtle" />
        {#each stageChips as chip (chip.key)}
          {#if chip.count === null || chip.count > 0 || stage === chip.key}
            <button
              onclick={() => setStage(chip.key)}
              aria-pressed={stage === chip.key}
              class="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-md px-2.5 text-[12px] transition-colors
                     {stage === chip.key
                ? 'bg-brand font-semibold text-brand-fg'
                : 'border border-line bg-surface font-medium text-muted hover:text-foreground'}"
            >
              {#if isStage(chip.key)}
                <span class="h-2 w-2 rounded-full {stageBadgeClass(chip.key)}"></span>
              {/if}
              {chip.label}
              {#if chip.count !== null}
                <span class="tabular-nums opacity-70">{chip.count}</span>
              {/if}
            </button>
          {/if}
        {/each}
      </div>
    </div>
  {/if}

  <div class="min-h-0 flex-1">
    {#if feed.loading}
      <div class="p-2"><RowSkeleton count={8} /></div>
    {:else if feed.saved.length === 0}
      <EmptyState
        icon={Bookmark}
        title="Nothing saved yet"
        body="Save any role, story or event and it stays here — even after the listing ages out of the feed. Saved roles get an application tracker."
        actionLabel="Browse roles"
        onAction={() => goto("/internships")}
      />
    {:else if list.length === 0}
      <EmptyState
        icon={tracking ? ListChecks : Bookmark}
        title={tracking ? "No applications in play" : "Nothing here"}
        body={tracking
          ? "Open a saved role and mark it Applied — it will be tracked here through interviews and offers."
          : "You have saved items, just none that match this view."}
        actionLabel="Show everything saved"
        onAction={() => {
          kind = "All";
          setStage("any");
        }}
      />
    {:else}
      <div class="flex h-full min-h-0">
        <div class="pane w-full lg:w-[400px] lg:shrink-0 lg:border-r lg:border-line">
          {#each visible as item (item.url)}
            <ItemRow
              {item}
              variant={wide ? "dense" : "band"}
              lead={isRole(item) ? "auto" : "type"}
              meta={metaFor(item)}
              dismissable={false}
              selected={selected?.url === item.url}
              onOpen={(i) => (selected = i)}
            />
          {/each}
          <InfiniteScroll {shown} total={list.length} noun="items" onMore={() => (shown += PAGE)} />
        </div>
        {#if wide && selected}
          <section class="flex min-w-0 flex-1 flex-col" aria-label="Saved item">
            {#key selected.url}
              <ItemDetail item={selected} dismissable={false} />
            {/key}
          </section>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if !wide}
  <ItemModal item={selected} onClose={() => (selected = null)} dismissable={false} />
{/if}
