<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import Band from "$lib/components/Band.svelte";
  import EmptyState from "$lib/components/EmptyState.svelte";
  import HeroMatch from "$lib/components/HeroMatch.svelte";
  import ItemModal from "$lib/components/ItemModal.svelte";
  import ItemRow from "$lib/components/ItemRow.svelte";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import RowSkeleton from "$lib/components/RowSkeleton.svelte";
  import { feed } from "$lib/feed.svelte";
  import { BookOpen, Briefcase, Calendar, Inbox, Sparkles } from "$lib/icons";

  // Today, not a feed.
  //
  // The old home page was the whole cache behind a search box, four selects, a
  // density switch and a discipline chip row — seven control clusters before
  // the first result, on a page whose job is to tell you what changed. This one
  // makes a single decision the hero, then shows three items of each kind, and
  // carries no filters at all. Filtering belongs on Roles, where you go
  // deliberately; searching belongs to the top bar, where every page can reach
  // it.

  /** Items per band before you ask for more. Three is a glance, not a scroll. */
  const PREVIEW = 3;
  /** How many more each "see all" reveals. Bands never render the whole cache. */
  const STEP = 12;

  let selected = $state<ScrapedItem | null>(null);
  let readingShown = $state(PREVIEW);
  let eventsShown = $state(PREVIEW);

  const roles = $derived(feed.internships);

  /** The hero: the best-ranked role you have not opened yet. */
  const hero = $derived(roles.find((i) => !feed.isSeen(i.url)) ?? roles[0] ?? null);

  const restOfRoles = $derived(roles.filter((i) => i.url !== hero?.url).slice(0, PREVIEW));

  const reading = $derived(feed.items.filter((i) => i.item_type === "Article"));

  /** Soonest first, and only what has not already happened. */
  const events = $derived(
    feed.items
      .filter((i) => i.item_type === "Event" && new Date(i.timestamp).getTime() >= Date.now())
      .sort((a, b) => a.timestamp.localeCompare(b.timestamp)),
  );

  /** What actually changed, which is the one thing a page called Today owes you. */
  const fresh = $derived.by(() => {
    const cutoff = Date.now() - 24 * 3600 * 1000;
    return feed.items.filter(
      (i) => !feed.isSeen(i.url) && new Date(i.timestamp).getTime() >= cutoff,
    ).length;
  });

  const subtitle = $derived(
    feed.loading
      ? `Scanning ${feed.status?.sources.length ?? 0} sources…`
      : fresh > 0
        ? `${fresh} new ${fresh === 1 ? "thing" : "things"} worth your attention since yesterday.`
        : `${feed.items.length.toLocaleString()} items cached · nothing new in the last day.`,
  );

  /** Steps a band open, then closes it again once it has shown everything. */
  function more(shown: number, total: number) {
    return shown >= total ? PREVIEW : Math.min(shown + STEP, total);
  }

  function bandAction(shown: number, total: number): string | undefined {
    if (total <= PREVIEW) return undefined;
    if (shown >= total) return "Show less";
    return shown === PREVIEW ? `See all ${total}` : "Show more";
  }

  /** How long until an event, which is the only thing anyone reads an event for. */
  function countdown(item: ScrapedItem): string {
    const days = Math.round((new Date(item.timestamp).getTime() - Date.now()) / 86400000);
    if (days <= 0) return "Today";
    if (days === 1) return "Tomorrow";
    return `In ${days} days`;
  }
</script>

<main class="w-full flex-1">
  <div class="mx-auto w-full max-w-6xl px-4 sm:px-6 lg:px-8">
    <PageHeader title="Today" {subtitle} />
  </div>

  {#if feed.loading}
    <div class="mx-auto w-full max-w-6xl px-4 pt-2 pb-8 sm:px-6 lg:px-8">
      <RowSkeleton count={8} />
    </div>
  {:else if feed.items.length === 0}
    <div class="mx-auto w-full max-w-6xl px-4 pt-2 pb-8 sm:px-6 lg:px-8">
      <EmptyState
        icon={Inbox}
        title="Nothing cached yet"
        body="Pull the latest listings, stories and events from your sources. It all stays on this device."
        actionLabel="Refresh now"
        busy={feed.refreshing}
        onAction={() => feed.refresh(true)}
      />
    </div>
  {:else}
    {#if hero}
      <HeroMatch item={hero} onOpen={(i) => (selected = i)} />
    {/if}

    <div class="mx-auto w-full max-w-6xl px-4 pb-8 sm:px-6 lg:px-8">
      <!-- One nudge, and only until it has been answered. Skills go first:
           it is the one that changes what every role on this page says. -->
      {#if feed.needsSkillsSetup}
        <button
          onclick={() => feed.openSkillsForm()}
          class="animate-fade-in mt-4 flex w-full items-center gap-2.5 rounded-xl border border-brand/30 bg-brand-soft px-4 py-3 text-left text-sm text-brand-soft-fg transition-colors hover:border-brand"
        >
          <Sparkles size={16} class="shrink-0" />
          <span>
            <span class="font-semibold">Tell us what you can build</span> and every role here will show
            how much of it you already cover.
          </span>
        </button>
      {:else if feed.interests.length === 0}
        <a
          href="/settings"
          class="animate-fade-in mt-4 flex w-full items-center gap-2.5 rounded-xl border border-brand/30 bg-brand-soft px-4 py-3 text-sm text-brand-soft-fg transition-colors hover:border-brand"
        >
          <Sparkles size={16} class="shrink-0" />
          <span>
            <span class="font-semibold">Pick your interests</span> to rank all of this around what you
            actually care about.
          </span>
        </a>
      {/if}

      {#if restOfRoles.length > 0}
        <Band
          icon={Briefcase}
          label="Roles"
          accent="text-job"
          blurb="ranked by how much of the posting you cover"
          actionLabel={roles.length > PREVIEW ? `See all ${roles.length}` : undefined}
          href="/internships"
        >
          {#each restOfRoles as item (item.url)}
            <ItemRow {item} onOpen={(i) => (selected = i)} />
          {/each}
        </Band>
      {/if}

      {#if reading.length > 0}
        <Band
          icon={BookOpen}
          label="Reading"
          accent="text-article"
          blurb="from the sources you follow"
          actionLabel={bandAction(readingShown, reading.length)}
          onAction={() => (readingShown = more(readingShown, reading.length))}
        >
          {#each reading.slice(0, readingShown) as item (item.url)}
            <ItemRow {item} lead="none" onOpen={(i) => (selected = i)} />
          {/each}
        </Band>
      {/if}

      {#if events.length > 0}
        <Band
          icon={Calendar}
          label="Happening"
          accent="text-event"
          blurb="deadlines first"
          actionLabel={bandAction(eventsShown, events.length)}
          onAction={() => (eventsShown = more(eventsShown, events.length))}
        >
          {#each events.slice(0, eventsShown) as item (item.url)}
            <ItemRow {item} meta={countdown(item)} onOpen={(i) => (selected = i)} />
          {/each}
        </Band>
      {/if}
    </div>
  {/if}
</main>

<ItemModal item={selected} onClose={() => (selected = null)} />
