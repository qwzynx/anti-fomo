<script lang="ts">
  import { timeAgo } from "$lib/api";
  import PageHeader from "$lib/components/PageHeader.svelte";
  import { feed } from "$lib/feed.svelte";
  import { DISCIPLINES } from "$lib/filters";
  import {
    Briefcase,
    Calendar,
    Check,
    FileText,
    GraduationCap,
    Monitor,
    Moon,
    Newspaper,
    RefreshCw,
    Square,
    RotateCcw,
    Sparkles,
    Sun,
    Trash2,
    skillCategoryIcon,
    type IconComponent,
  } from "$lib/icons";
  import type { ScrapePrefs, SourceCategory } from "$lib/api";
  import { groupByCategory } from "$lib/skills";
  import { theme, type Theme } from "$lib/theme.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import { toast } from "$lib/toast.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openLogDir } from "$lib/api";

  let version = $state<string | null>(null);
  $effect(() => {
    getVersion().then(
      (v) => (version = v),
      () => (version = null),
    );
  });

  async function openLogs() {
    try {
      await openLogDir();
    } catch (e) {
      toast.error("open the log folder", e);
    }
  }

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

  // There is no undo behind this, so it asks first — in the shared confirm
  // dialog, which focuses Cancel, so a stray Enter cannot be the thing that
  // deletes.
  let clearing = $state(false);

  async function clearData() {
    const saved = feed.status?.saved_count ?? 0;
    const ok = await confirm({
      title: "Delete everything collected on this device?",
      body: `Every cached posting and description${saved > 0 ? `, and your ${saved} saved item${saved === 1 ? "" : "s"},` : ""} will be deleted. Your field, interests, skills and résumés are kept. This cannot be undone.`,
      confirmLabel: "Delete everything",
      danger: true,
    });
    if (!ok) return;
    clearing = true;
    try {
      await feed.clearData();
    } finally {
      clearing = false;
    }
  }

  const skillGroups = $derived(groupByCategory(feed.skills, feed.skillCatalog));
  const sources = $derived(feed.status?.sources ?? []);
  // A source the reader switched off is not "quiet": it was never asked.
  const quiet = $derived(sources.filter((s) => s.enabled && s.count === 0).length);

  // What the reader can ask to have collected. The same four the setup form
  // asks, in the same words, so the two cannot disagree about what a kind is.
  const KINDS: { key: keyof Omit<ScrapePrefs, "disabled_sources">; label: string; icon: IconComponent }[] = [
    { key: "internships", label: "Internships", icon: GraduationCap },
    { key: "jobs", label: "Jobs", icon: Briefcase },
    { key: "news", label: "News", icon: Newspaper },
    { key: "events", label: "Events", icon: Calendar },
  ];
  const GROUPS: { key: SourceCategory; label: string }[] = [
    { key: "opportunities", label: "Roles" },
    { key: "news", label: "News" },
    { key: "events", label: "Events" },
  ];

  const grouped = $derived(
    GROUPS.map((g) => ({ ...g, sources: sources.filter((s) => s.category === g.key) })).filter(
      (g) => g.sources.length > 0,
    ),
  );

  /** Whether anything in a category is wanted at all: roles need either kind. */
  function categoryOn(category: SourceCategory): boolean {
    const p = feed.prefs;
    if (category === "opportunities") return p.internships || p.jobs;
    return category === "news" ? p.news : p.events;
  }

  function setKind(key: (typeof KINDS)[number]["key"], on: boolean) {
    void feed.setPrefs({ ...feed.prefs, [key]: on });
  }

  function setSource(name: string, on: boolean) {
    const rest = feed.prefs.disabled_sources.filter((s) => s !== name);
    void feed.setPrefs({ ...feed.prefs, disabled_sources: on ? rest : [...rest, name] });
  }
</script>

<!-- Reached from the top bar's tool cluster at md: and up, and from the fourth
     tab on a phone. It is no longer one of the app's destinations: it is
     somewhere you go to change something and then leave. -->
<div class="flex h-full min-h-0 flex-col">
  <PageHeader
    title="Settings"
    subtitle="Everything is stored locally on this device. No account, no server."
  />

  <div class="pane flex-1">
  <div class="mx-auto w-full max-w-3xl px-4 py-5 sm:px-6">

  <section class="card mb-4 p-4">
    <h2 class="mb-1 text-[14px] font-semibold">Your field</h2>
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

  <section class="card mb-4 p-4">
    <div class="mb-1 flex items-baseline justify-between gap-3">
      <h2 class="text-[14px] font-semibold">Your interests</h2>
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

  <section class="card mb-4 p-4">
    <div class="mb-1 flex items-baseline justify-between gap-3">
      <h2 class="text-[14px] font-semibold">Your skills</h2>
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
      class="inline-flex items-center gap-1.5 rounded-md bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
    >
      <Sparkles size={15} strokeWidth={2.5} />
      {feed.needsSkillsSetup ? "Set up your skills" : "Edit your skills"}
    </button>
  </section>

  <section class="card mb-4 p-4">
    <h2 class="mb-1 text-[14px] font-semibold">Appearance</h2>
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

  <section class="card mb-4 p-4">
    <div class="mb-4 flex items-start justify-between gap-4">
      <div>
        <h2 class="mb-1 text-[14px] font-semibold">What to scrape</h2>
        <p class="text-sm text-muted">
          {#if feed.status}
            {feed.status.item_count.toLocaleString()} items cached · last refreshed
            {feed.status.last_refresh ? timeAgo(feed.status.last_refresh) : "never"}
          {:else}
            Loading…
          {/if}
          {#if quiet > 0}
            · <span class="text-star">{quiet} quiet</span>
          {/if}
        </p>
      </div>
      <div class="flex shrink-0 flex-col items-end gap-1.5">
        {#if feed.refreshing}
          <button
            onclick={() => feed.stop()}
            disabled={feed.phase === "stopping"}
            class="inline-flex items-center gap-1.5 rounded-md border border-line px-4 py-2 text-sm font-semibold text-foreground transition-colors hover:border-subtle disabled:opacity-50"
          >
            <Square size={13} class="fill-current" />
            {feed.phase === "stopping" ? "Stopping…" : "Stop scraping"}
          </button>
        {:else}
          <button
            onclick={() => feed.refresh(true)}
            class="inline-flex items-center gap-1.5 rounded-md bg-brand px-4 py-2 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
          >
            <RefreshCw size={15} />
            Scrape everything
          </button>
          <button
            onclick={() => feed.refresh(false)}
            class="text-xs font-semibold text-subtle transition-colors hover:text-foreground"
            title="Only the sources whose last run is out of date"
          >
            Only what's due
          </button>
        {/if}
      </div>
    </div>

    <p class="mb-2 text-xs font-bold tracking-wide text-subtle uppercase">Collect</p>
    <div class="mb-2 flex flex-wrap gap-2">
      {#each KINDS as kind (kind.key)}
        {@const on = feed.prefs[kind.key]}
        {@const Icon = kind.icon}
        <button
          type="button"
          aria-pressed={on}
          onclick={() => setKind(kind.key, !on)}
          class="chip inline-flex items-center gap-1.5 px-4 py-2 text-sm {on
            ? 'bg-brand text-brand-fg'
            : 'border border-line text-muted hover:border-brand'}"
        >
          <Icon size={14} />
          {kind.label}
        </button>
      {/each}
    </div>
    <p class="mb-5 text-xs text-subtle">
      Switching one off deletes what it collected and stops contacting its sources. Switching one on
      fetches just that.
    </p>

    {#if grouped.length > 0}
      {#each grouped as group (group.key)}
        {@const groupOn = categoryOn(group.key)}
        <p class="mb-1 text-xs font-bold tracking-wide text-subtle uppercase">{group.label}</p>
        <ul class="mb-4 flex flex-col divide-y divide-line-soft last:mb-0">
          {#each group.sources as source (source.name)}
            {@const on = !feed.prefs.disabled_sources.includes(source.name)}
            <li class="flex items-center justify-between gap-3 py-2 text-sm">
              <span class="flex min-w-0 items-center gap-2 font-medium">
                <!-- A source contributing nothing is the signal that a scraper has broken;
                     it stays listed rather than disappearing. -->
                <span
                  class="h-1.5 w-1.5 shrink-0 rounded-full {!source.enabled
                    ? 'bg-line'
                    : source.failing || source.count === 0
                      ? 'bg-star'
                      : 'bg-job'}"
                  aria-hidden="true"
                ></span>
                <span class="truncate {source.enabled ? '' : 'text-subtle'}">{source.name}</span>
              </span>
              <span class="flex shrink-0 items-center gap-3">
                <span
                  class="max-w-[14rem] truncate text-right text-xs {source.failing
                    ? 'text-star'
                    : 'text-subtle'}"
                  title={source.last_error ?? undefined}
                >
                  {#if !source.enabled}
                    off
                  {:else if source.failing}
                    didn't answer — will retry
                  {:else if source.last_error && source.count === 0}
                    no longer updated
                  {:else if source.count > 0}
                    {source.count.toLocaleString()} items
                  {:else}
                    nothing returned
                  {/if}
                </span>
                <button
                  type="button"
                  role="switch"
                  aria-checked={on && groupOn}
                  aria-label="Collect from {source.name}"
                  disabled={!groupOn}
                  onclick={() => setSource(source.name, !on)}
                  class="relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:opacity-40 {on &&
                  groupOn
                    ? 'bg-brand'
                    : 'bg-line'}"
                >
                  <span
                    class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-surface shadow transition-transform {on &&
                    groupOn
                      ? 'translate-x-4'
                      : ''}"
                  ></span>
                </button>
              </span>
            </li>
          {/each}
        </ul>
      {/each}
    {:else}
      <p class="text-sm text-muted">Nothing cached yet.</p>
    {/if}

    <div class="mt-4 border-t border-line-soft pt-4">
      <button
        type="button"
        onclick={() => feed.openSetup()}
        class="inline-flex items-center gap-1.5 rounded-md border border-line px-4 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-foreground"
      >
        <Sparkles size={14} />
        Run setup again
      </button>
    </div>
  </section>

  {#if (feed.status?.dismissed_count ?? 0) > 0}
    <section class="card mb-4 p-4">
      <h2 class="mb-1 text-[14px] font-semibold">Hidden items</h2>
      <p class="mb-4 text-sm text-muted">
        You've hidden {feed.status?.dismissed_count} item{feed.status?.dismissed_count === 1
          ? ""
          : "s"}. Restoring brings them all back to the feed.
      </p>
      <button
        onclick={() => feed.restoreDismissed()}
        class="inline-flex items-center gap-1.5 rounded-md border border-line px-4 py-2 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-brand"
      >
        <RotateCcw size={15} />
        Restore hidden items
      </button>
    </section>
  {/if}

  <section class="card mb-4 border-danger/30 p-4">
    <h2 class="mb-1 text-[14px] font-semibold">Clear stored data</h2>
    <p class="mb-4 text-sm text-muted">
      Deletes everything the app has collected on this device — {feed.status?.item_count ?? 0}
      cached item{(feed.status?.item_count ?? 0) === 1 ? "" : "s"}, every fetched job
      description, and all
      {feed.status?.saved_count ?? 0} saved item{(feed.status?.saved_count ?? 0) === 1 ? "" : "s"}
      along with what you've hidden and read. Your field, interests and skills are kept, and the
      feed refills on the next refresh. This cannot be undone.
    </p>

    <button
      onclick={clearData}
      disabled={clearing}
      class="inline-flex items-center gap-1.5 rounded-md border border-danger/40 px-4 py-2 text-sm font-semibold text-danger transition-colors hover:bg-danger hover:text-danger-fg disabled:opacity-50"
    >
      <Trash2 size={15} />
      {clearing ? "Clearing…" : "Clear all data"}
    </button>
  </section>

  <section class="card p-4">
    <h2 class="mb-1 text-[14px] font-semibold">About</h2>
    <p class="text-sm leading-relaxed text-muted">
      Anti-FOMO scrapes {sources.length} public sources directly on your device and ranks them against
      your field and interests. Results are cached locally, so the feed opens instantly and stays readable
      offline. Nothing you save or write leaves this device; company logos are loaded from Google's
      favicon service.
    </p>
    <div class="mt-4 flex flex-wrap items-center gap-3 border-t border-line-soft pt-4">
      <span class="text-sm text-muted">
        Version <span class="font-semibold text-foreground tabular-nums">{version ?? "…"}</span>
      </span>
      <button
        onclick={openLogs}
        class="inline-flex items-center gap-1.5 rounded-md border border-line px-3 py-1.5 text-sm font-semibold text-muted transition-colors hover:border-brand hover:text-foreground"
      >
        <FileText size={14} />
        Open log folder
      </button>
      <span class="text-xs text-subtle">Attach the latest log to a bug report.</span>
    </div>
  </section>
  </div>
  </div>
</div>
