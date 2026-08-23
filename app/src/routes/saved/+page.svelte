<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import Band from "$lib/components/Band.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import { feed } from "$lib/feed.svelte";
  import { BookOpen, Bookmark, Briefcase, Calendar, type IconComponent } from "$lib/icons";

  // Saved reads as three shelves rather than one undifferentiated list: a role
  // you kept, an article you kept and a deadline you kept are three different
  // promises to yourself, and stacking them in timestamp order buries the one
  // with a date on it. The type chips still narrow to one shelf, but the
  // default view shows all three at once because that is the whole point of
  // opening the page.

  /** Rows per shelf before the shelf offers the rest. */
  const PREVIEW = 20;

  type Group = {
    key: string;
    label: string;
    icon: IconComponent;
    accent: string;
    types: ScrapedItem["item_type"][];
    blurb: (items: ScrapedItem[]) => string | undefined;
  };

  const GROUPS: Group[] = [
    {
      key: "Roles",
      label: "Roles",
      icon: Briefcase,
      accent: "text-job",
      types: ["Internship", "Job"],
      blurb: () => "kept for when you have time to apply",
    },
    {
      key: "Reading",
      label: "Reading",
      icon: BookOpen,
      accent: "text-article",
      types: ["Article"],
      blurb: () => "kept for later",
    },
    {
      key: "Events",
      label: "Events",
      icon: Calendar,
      accent: "text-event",
      types: ["Event"],
      blurb: (items) => {
        const soon = items.filter(
          (i) => new Date(i.timestamp).getTime() - Date.now() < 7 * 86400000,
        ).length;
        return soon > 0 ? `${soon} within the week` : "nothing closing this week";
      },
    },
  ];

  let selected = $state<ScrapedItem | null>(null);
  let active = $state("All");
  let shown = $state<Record<string, number>>({});

  const shelves = $derived(
    GROUPS.map((g) => ({
      ...g,
      items: feed.saved.filter((i) => g.types.includes(i.item_type)),
    })).filter((g) => g.items.length > 0),
  );

  const visible = $derived(active === "All" ? shelves : shelves.filter((g) => g.key === active));

  /** Only offer a chip for a shelf that has something on it. */
  const chips = $derived([
    { key: "All", label: "All", count: feed.saved.length },
    ...shelves.map((g) => ({ key: g.key, label: g.label, count: g.items.length })),
  ]);

  const subtitle = $derived(
    feed.saved.length === 0
      ? "Nothing saved yet."
      : `${feed.saved.length} ${feed.saved.length === 1 ? "item" : "items"}, kept on this device even after they drop out of the feed.`,
  );

  /**
   * What an event's column says. Roles and articles fall through to the row's
   * own default — the location and the source — because the coverage badge
   * already carries the other half of what a role would say here, and the row
   * briefly read "You cover 4/6" directly beside a badge reading "4/6".
   * `item_state` records no saved-at time, so nothing here can say when.
   */
  function metaFor(item: ScrapedItem): string | undefined {
    if (item.item_type !== "Event") return undefined;
    const days = Math.round((new Date(item.timestamp).getTime() - Date.now()) / 86400000);
    if (days < 0) return "Already passed";
    return days === 0 ? "Today" : days === 1 ? "Tomorrow" : `In ${days} days`;
  }
</script>

<main class="mx-auto w-full max-w-6xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <PageHeader title="Saved" {subtitle} />

  {#if feed.saved.length === 0}
    <EmptyState
      icon={Bookmark}
      title="No saved items"
      body="Tap the bookmark on any row to keep it here. Saved items stick around even after the listing ages out of the feed."
    />
  {:else}
    {#if chips.length > 2}
      <div class="scrollbar-hide -mx-4 overflow-x-auto px-4 pb-1 sm:mx-0 sm:px-0">
        <div class="flex gap-2">
          {#each chips as chip (chip.key)}
            <button
              onclick={() => (active = chip.key)}
              aria-pressed={active === chip.key}
              class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full px-3.5 text-[13px] transition-colors
                     {active === chip.key
                ? 'bg-brand font-semibold text-brand-fg'
                : 'border border-line bg-surface font-medium text-muted hover:border-subtle'}"
            >
              {chip.label}
              <span class="tabular-nums {active === chip.key ? 'opacity-80' : 'text-subtle'}">
                {chip.count}
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#each visible as shelf (shelf.key)}
      {@const limit = shown[shelf.key] ?? PREVIEW}
      <Band
        icon={shelf.icon}
        label={shelf.label}
        accent={shelf.accent}
        blurb={shelf.blurb(shelf.items)}
        actionLabel={shelf.items.length > limit ? `Show all ${shelf.items.length}` : undefined}
        onAction={() => (shown = { ...shown, [shelf.key]: shelf.items.length })}
      >
        {#each shelf.items.slice(0, limit) as item (item.url)}
          <!-- Dismissing something you deliberately saved is contradictory, so
               the row offers un-saving and nothing else. -->
          <ItemRow
            {item}
            lead="type"
            meta={metaFor(item)}
            dismissable={false}
            onOpen={(i) => (selected = i)}
          />
        {/each}
      </Band>
    {/each}

    {#if visible.length === 0}
      <EmptyState
        icon={Bookmark}
        title="Nothing of that type"
        body="You have saved items, just none in this category."
        actionLabel="Show all"
        onAction={() => (active = "All")}
      />
    {/if}
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
