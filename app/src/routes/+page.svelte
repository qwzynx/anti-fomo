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
  import { BookOpen, Briefcase, Calendar, Flame, Inbox, ListChecks, Sparkles } from "$lib/icons";
  import { closesInDays, eventDate } from "$lib/item";

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
  /** The most last calls the band will ever list; Rust sends more than a glance needs. */
  const LAST_CALL_CAP = 8;

  let selected = $state<ScrapedItem | null>(null);
  let readingShown = $state(PREVIEW);
  let eventsShown = $state(PREVIEW);
  let lastCallShown = $state(PREVIEW);

  const roles = $derived(feed.internships);

  /** The hero: the best-ranked role you have not opened yet. */
  const hero = $derived(roles.find((i) => !feed.isSeen(i.url)) ?? roles[0] ?? null);

  // The hero is the best match and a last call is the most urgent one; they may well be the same
  // posting, and that is fine. What is not worth doing is listing a role a third time just below.
  const restOfRoles = $derived.by(() => {
    const shown = new Set(lastCall.slice(0, lastCallShown).map((i) => i.url));
    return roles.filter((i) => i.url !== hero?.url && !shown.has(i.url)).slice(0, PREVIEW);
  });

  /**
   * Strong postings that are about to close. Rust ranks "about to" against the clock at the last
   * re-rank, which can be up to an hour old, so a deadline that has since passed is dropped here
   * rather than shown as a last call it no longer is.
   */
  const lastCall = $derived(
    feed.lastCall
      .filter((i) => (closesInDays(i) ?? -1) >= 0)
      .slice(0, LAST_CALL_CAP),
  );

  const reading = $derived(feed.items.filter((i) => i.item_type === "Article"));

  /**
   * Soonest first, and only what has not already happened — by the date the event is *about*: a
   * hackathon's deadline, not the day entries opened, which is what its `timestamp` says.
   */
  const events = $derived(
    feed.items
      .filter((i) => i.item_type === "Event" && new Date(eventDate(i)).getTime() >= Date.now())
      .sort((a, b) => eventDate(a).localeCompare(eventDate(b))),
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
      ? "Loading your feed…"
      : fresh > 0
        ? `${fresh} new ${fresh === 1 ? "thing" : "things"} worth your attention since yesterday.`
        : `${feed.items.length.toLocaleString()} items cached · nothing new in the last day.`,
  );

  /** The tiles across the top: four numbers worth a glance, each a way in. */
  const stats = $derived([
    { label: "New since yesterday", value: fresh, icon: Sparkles, accent: "text-brand", href: "/internships" },
    { label: "Open roles", value: roles.length, icon: Briefcase, accent: "text-job", href: "/internships" },
    { label: "Closing this week", value: lastCall.length, icon: Flame, accent: "text-danger", href: "/internships" },
    {
      label: "Applications in play",
      value: feed.activeApplications,
      icon: ListChecks,
      accent: "text-event",
      href: "/saved?stage=active",
    },
  ]);

  /** Steps a band open, then closes it again once it has shown everything. */
  function more(shown: number, total: number) {
    return shown >= total ? PREVIEW : Math.min(shown + STEP, total);
  }

  function bandAction(shown: number, total: number): string | undefined {
    if (total <= PREVIEW) return undefined;
    if (shown >= total) return "Show less";
    return shown === PREVIEW ? `See all ${total}` : "Show more";
  }

  /**
   * How long until an event, which is the only thing anyone reads an event for. Worded as a
   * deadline when it is one — "Closes in 4 days" — to match what a role's row says.
   */
  function countdown(item: ScrapedItem): string {
    const days = Math.round((new Date(eventDate(item)).getTime() - Date.now()) / 86400000);
    const when = days <= 0 ? "today" : days === 1 ? "tomorrow" : `in ${days} days`;
    if (item.closes_at) return `Closes ${when}`;
    return when.charAt(0).toUpperCase() + when.slice(1);
  }
</script>

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title="Today" {subtitle} />

  <div class="pane flex-1">
    {#if feed.loading}
      <div class="p-2"><RowSkeleton count={10} /></div>
    {:else if feed.items.length === 0}
      <EmptyState
        icon={Inbox}
        title={feed.refreshing ? "Collecting your feed…" : "Nothing cached yet"}
        body={feed.refreshing
          ? "Results appear here as each source finishes. You can leave this open, or stop it from the sidebar."
          : "Pull the latest listings, stories and events from your sources. It all stays on this device."}
        actionLabel={feed.refreshing ? "Stop" : "Refresh now"}
        onAction={() => (feed.refreshing ? feed.stop() : feed.refresh(true))}
      />
    {:else}
      <div class="mx-auto w-full max-w-6xl px-4 py-5 sm:px-6">
        <!-- At a glance: what is new, what is closing, what you are in the middle of. -->
        <div class="mb-6 grid grid-cols-2 gap-2.5 lg:grid-cols-4">
          {#each stats as stat (stat.label)}
            <a
              href={stat.href}
              class="group rounded-lg border border-line bg-surface px-3.5 py-3 transition-colors hover:border-subtle/50"
            >
              <p class="flex items-center gap-1.5 text-[12px] text-subtle">
                <stat.icon size={13} class="shrink-0 {stat.accent}" />
                {stat.label}
              </p>
              <p class="mt-1 text-[22px] leading-none font-semibold tabular-nums">
                {stat.value.toLocaleString()}
              </p>
            </a>
          {/each}
        </div>

        <!-- One nudge, and only until it has been answered. Skills go first:
             it is the one that changes what every role on this page says. -->
        {#if feed.needsSkillsSetup}
          <button
            onclick={() => feed.openSkillsForm()}
            class="animate-fade-in mb-6 flex w-full items-center gap-2.5 rounded-lg border border-brand/30 bg-brand-soft px-4 py-2.5 text-left text-[13px] text-brand-soft-fg transition-colors hover:border-brand"
          >
            <Sparkles size={15} class="shrink-0" />
            <span>
              <span class="font-semibold">Tell us what you can build</span> and every role here will show
              how much of it you already cover.
            </span>
          </button>
        {:else if feed.interests.length === 0}
          <a
            href="/settings"
            class="animate-fade-in mb-6 flex w-full items-center gap-2.5 rounded-lg border border-brand/30 bg-brand-soft px-4 py-2.5 text-[13px] text-brand-soft-fg transition-colors hover:border-brand"
          >
            <Sparkles size={15} class="shrink-0" />
            <span>
              <span class="font-semibold">Pick your interests</span> to rank all of this around what you
              actually care about.
            </span>
          </a>
        {/if}

        {#if hero}
          <HeroMatch item={hero} onOpen={(i) => (selected = i)} />
        {/if}

        <div class="grid gap-x-6 xl:grid-cols-2">
          <div class="min-w-0">
            <!-- First on purpose: the one section whose contents expire. -->
            {#if lastCall.length > 0}
              <Band
                icon={Flame}
                label="Last call"
                accent="text-danger"
                blurb="strong matches closing within a week"
                actionLabel={bandAction(lastCallShown, lastCall.length)}
                onAction={() => (lastCallShown = more(lastCallShown, lastCall.length))}
              >
                {#each lastCall.slice(0, lastCallShown) as item (item.url)}
                  <ItemRow
                    {item}
                    variant="dense"
                    meta={item.item_type === "Event" ? countdown(item) : undefined}
                    onOpen={(i) => (selected = i)}
                  />
                {/each}
              </Band>
            {/if}

            {#if restOfRoles.length > 0}
              <Band
                icon={Briefcase}
                label="Top roles"
                accent="text-job"
                blurb="ranked by how much you cover"
                actionLabel={roles.length > PREVIEW ? `All ${roles.length.toLocaleString()}` : undefined}
                href="/internships"
              >
                {#each restOfRoles as item (item.url)}
                  <ItemRow {item} variant="dense" onOpen={(i) => (selected = i)} />
                {/each}
              </Band>
            {/if}
          </div>

          <div class="min-w-0">
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
                  <ItemRow {item} variant="dense" meta={countdown(item)} onOpen={(i) => (selected = i)} />
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
                  <ItemRow {item} variant="dense" lead="none" onOpen={(i) => (selected = i)} />
                {/each}
              </Band>
            {/if}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<ItemModal item={selected} onClose={() => (selected = null)} />
