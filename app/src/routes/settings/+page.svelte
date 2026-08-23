<script lang="ts">
  import { timeAgo } from "$lib/api";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import { feed } from "$lib/feed.svelte";
  import { DISCIPLINES } from "$lib/filters";
  import {
    Check,
    Monitor,
    Moon,
    RefreshCw,
    RotateCcw,
    Sparkles,
    Sun,
    Trash2,
    skillCategoryIcon,
  } from "$lib/icons";
  import { groupByCategory } from "$lib/skills";
  import { theme, type Theme } from "$lib/theme.svelte";

  const MAJORS = DISCIPLINES.filter((d) => d !== "All");
  const THEMES: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon },
    { value: "system", label: "System", icon: Monitor },
  ];

  function toggleInterest(name: string) {
    const next = feed.interests.includes(name)
      ? feed.interests.filter((i) => i !== name)
      : [...feed.interests, name];
    feed.setInterests(next);
  }

  // Two taps to wipe the store: the button arms, and a second, separate
  // button commits. There is no undo behind it, so a stray tap must not be
  // enough on its own.
  let confirmingClear = $state(false);
  let clearing = $state(false);

  async function clearData() {
    clearing = true;
    try {
      await feed.clearData();
    } finally {
      clearing = false;
      confirmingClear = false;
    }
  }

  const skillGroups = $derived(groupByCategory(feed.skills, feed.skillCatalog));
  const sources = $derived(feed.status?.sources ?? []);
  const quiet = $derived(sources.filter((s) => s.count === 0).length);
</script>

<!-- Reached from the top bar's tool cluster at md: and up, and from the fourth
     tab on a phone. It is no longer one of the app's destinations: it is
     somewhere you go to change something and then leave. -->
<main class="mx-auto w-full max-w-3xl flex-1 px-4 pb-8 sm:px-6 lg:px-8">
  <PageHeader
    title="Settings"
    subtitle="Everything is stored locally on this device. No account, no server."
  />

  <section class="card mt-4 mb-5 p-5">
    <h3 class="mb-1 text-lg font-bold">Your field</h3>
    <p class="mb-4 text-sm text-muted">Items matching your field are ranked to the top.</p>
    <div class="flex flex-wrap gap-2">
      {#each MAJORS as option (option)}
        <button
          onclick={() => feed.setMajor(option)}
          aria-pressed={feed.major === option}
          class="chip px-4 py-2 text-sm {feed.major === option
            ? 'bg-brand text-brand-fg'
            : 'border border-line text-muted hover:border-brand'}"
        >
          {option}
        </button>
      {/each}
    </div>
  </section>

  <section class="card mb-5 p-5">
    <div class="mb-1 flex items-baseline justify-between gap-3">
      <h3 class="text-lg font-bold">Your interests</h3>
      <span class="shrink-0 text-xs font-medium text-subtle">
        {feed.interests.length} selected
      </span>
    </div>
    <p class="mb-4 text-sm text-muted">
      Your field decides what qualifies; these decide what rises to the top within it. Matching
      items are boosted and show the tag that fired.
    </p>
    <div class="flex flex-wrap gap-2">
      {#each feed.availableInterests as name (name)}
        {@const active = feed.interests.includes(name)}
        <button
          onclick={() => toggleInterest(name)}
          aria-pressed={active}
          class="chip inline-flex items-center gap-1.5 px-3.5 py-2 text-sm {active
            ? 'bg-brand text-brand-fg'
            : 'border border-line text-muted hover:border-brand'}"
        >
          {#if active}<Check size={14} strokeWidth={3} />{/if}
          {name}
        </button>
      {/each}
    </div>
    {#if feed.interests.length > 0}
      <button
        onclick={() => feed.setInterests([])}
        class="mt-4 text-xs font-semibold text-brand hover:underline"
      >
        Clear all
      </button>
    {/if}
  </section>

  <section class="card mb-5 p-5">
    <div class="mb-1 flex items-baseline justify-between gap-3">
      <h3 class="text-lg font-bold">Your skills</h3>
      <span class="shrink-0 text-xs font-medium text-subtle">
        {feed.skills.length} selected
      </span>
    </div>
    <p class="mb-4 text-sm text-muted">
      What you can actually build. Every job is matched against the skills its own posting
      asks for, so you can see how much of it you already cover — and the ones you fit
      rank higher.
    </p>

    {#if skillGroups.length > 0}
      <div class="mb-4 flex flex-col gap-2.5">
        {#each skillGroups as group (group.name)}
          {@const Icon = skillCategoryIcon(group.name)}
          <div class="flex flex-wrap items-center gap-1.5">
            <span
              class="mr-1 inline-flex shrink-0 items-center gap-1 text-[11px] font-bold tracking-wide text-subtle uppercase"
            >
              <Icon size={11} />
              {group.name}
            </span>
            {#each group.skills as skill (skill)}
              <span
                class="rounded-md bg-line-soft px-2 py-0.5 text-[11px] font-semibold text-muted"
              >
                {skill}
              </span>
            {/each}
          </div>
        {/each}
      </div>
    {/if}

    <button
      onclick={() => feed.openSkillsForm()}
      class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
    >
      <Sparkles size={15} strokeWidth={2.5} />
      {feed.needsSkillsSetup ? "Set up your skills" : "Edit your skills"}
    </button>
  </section>

  <section class="card mb-5 p-5">
    <h3 class="mb-1 text-lg font-bold">Appearance</h3>
    <p class="mb-4 text-sm text-muted">"System" follows your OS setting and updates live.</p>
    <div class="flex flex-wrap gap-2">
      {#each THEMES as option (option.value)}
        {@const Icon = option.icon}
        <button
          onclick={() => theme.set(option.value)}
          aria-pressed={theme.value === option.value}
          class="chip inline-flex items-center gap-1.5 px-4 py-2 text-sm {theme.value ===
          option.value
            ? 'bg-brand text-brand-fg'
            : 'border border-line text-muted hover:border-brand'}"
        >
          <Icon size={15} />
          {option.label}
        </button>
      {/each}
    </div>
  </section>

  <section class="card mb-5 p-5">
    <div class="mb-4 flex items-start justify-between gap-4">
      <div>
        <h3 class="mb-1 text-lg font-bold">Sources</h3>
        <p class="text-sm text-muted">
          {feed.status?.item_count ?? 0} items cached · last refreshed
          {feed.status?.last_refresh ? timeAgo(feed.status.last_refresh) : "never"}
          {#if quiet > 0}
            · <span class="text-star">{quiet} quiet</span>
          {/if}
        </p>
      </div>
      <button
        onclick={() => feed.refresh(true)}
        disabled={feed.refreshing}
        class="inline-flex shrink-0 items-center gap-1.5 rounded-xl bg-brand px-4 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
      >
        <RefreshCw size={15} class={feed.refreshing ? "animate-spin" : ""} />
        {feed.refreshing ? "Refreshing…" : "Refresh now"}
      </button>
    </div>

    {#if sources.length > 0}
      <ul class="flex flex-col divide-y divide-line-soft">
        {#each sources as source (source.name)}
          <li class="flex items-center justify-between py-2 text-sm">
            <span class="flex items-center gap-2 font-medium">
              <!-- A registered source contributing nothing is the signal that a
                   scraper has broken; it stays listed rather than disappearing. -->
              <span
                class="h-1.5 w-1.5 rounded-full {source.count > 0 ? 'bg-job' : 'bg-star'}"
                aria-hidden="true"
              ></span>
              {source.name}
            </span>
            <span class="text-subtle">
              {source.count > 0 ? `${source.count} items` : "nothing returned"}
            </span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="text-sm text-muted">Nothing cached yet.</p>
    {/if}
  </section>

  {#if (feed.status?.dismissed_count ?? 0) > 0}
    <section class="card mb-5 p-5">
      <h3 class="mb-1 text-lg font-bold">Hidden items</h3>
      <p class="mb-4 text-sm text-muted">
        You've hidden {feed.status?.dismissed_count} item{feed.status?.dismissed_count === 1
          ? ""
          : "s"}. Restoring brings them all back to the feed.
      </p>
      <button
        onclick={() => feed.restoreDismissed()}
        class="inline-flex items-center gap-1.5 rounded-xl border border-line px-4 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-brand"
      >
        <RotateCcw size={15} />
        Restore hidden items
      </button>
    </section>
  {/if}

  <section class="card mb-5 border-danger/30 p-5">
    <h3 class="mb-1 text-lg font-bold">Clear stored data</h3>
    <p class="mb-4 text-sm text-muted">
      Deletes everything the app has collected on this device — {feed.status?.item_count ?? 0}
      cached item{(feed.status?.item_count ?? 0) === 1 ? "" : "s"}, every fetched job
      description, and all
      {feed.status?.saved_count ?? 0} saved item{(feed.status?.saved_count ?? 0) === 1 ? "" : "s"}
      along with what you've hidden and read. Your field, interests and skills are kept, and the
      feed refills on the next refresh. This cannot be undone.
    </p>

    {#if confirmingClear}
      <div class="flex flex-wrap items-center gap-2">
        <button
          onclick={clearData}
          disabled={clearing}
          class="inline-flex items-center gap-1.5 rounded-xl bg-danger px-4 py-2 text-sm font-semibold text-danger-fg transition-colors hover:opacity-90 disabled:opacity-50"
        >
          <Trash2 size={15} />
          {clearing ? "Clearing…" : "Yes, delete everything"}
        </button>
        <button
          onclick={() => (confirmingClear = false)}
          disabled={clearing}
          class="rounded-xl border border-line px-4 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-brand disabled:opacity-50"
        >
          Cancel
        </button>
      </div>
    {:else}
      <button
        onclick={() => (confirmingClear = true)}
        class="inline-flex items-center gap-1.5 rounded-xl border border-danger/40 px-4 py-2 text-sm font-semibold text-danger transition-colors hover:bg-danger hover:text-danger-fg"
      >
        <Trash2 size={15} />
        Clear all data
      </button>
    {/if}
  </section>

  <section class="card p-5">
    <h3 class="mb-1 text-lg font-bold">About</h3>
    <p class="text-sm leading-relaxed text-muted">
      Anti-FOMO scrapes {sources.length} public sources directly on your device and ranks them against
      your field and interests. Results are cached locally, so the feed opens instantly and stays readable
      offline.
    </p>
  </section>
</main>
