<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import {
    ArrowLeft,
    Briefcase,
    Calendar,
    ChevronRight,
    CircleAlert,
    CircleCheck,
    GraduationCap,
    LoaderCircle,
    MapPin,
    Minus,
    Newspaper,
    Sparkles,
    X,
    type IconComponent,
  } from "$lib/icons";

  // The setup form: what to collect, before anything is collected.
  //
  // The app used to scrape every source the moment it opened, whatever the
  // reader cared about. This asks first, and the answers decide which requests
  // are made at all — switching news off means the news sources are never
  // contacted, not that their stories are hidden.
  //
  // Three questions and then the scrape itself, on the same screen, so what you
  // just chose is visibly what is being fetched:
  //   0  what to keep up with        (the four kinds of thing)
  //   1  what you are interested in  (the same tags that rank the feed)
  //   2  where, and at what level    (a region, and the bands you are after)
  //   3  the scrape, source by source
  //
  // Reachable three ways. A first launch cannot leave it: nothing has been
  // fetched and there is nowhere else to go. An existing install is *offered*
  // it once and may skip. And Settings can raise it again whenever, which is
  // why it is not a `Modal` — a first-run gate must not close on a stray Escape
  // or a click on the backdrop and strand an empty feed.

  type Kind = "internships" | "jobs" | "news" | "events";

  const KINDS: { key: Kind; label: string; blurb: string; icon: IconComponent }[] = [
    {
      key: "internships",
      label: "Internships & co-ops",
      blurb: "Student roles, from the community lists and employers' own boards.",
      icon: GraduationCap,
    },
    {
      key: "jobs",
      label: "Jobs",
      blurb: "New-grad and experienced roles, read straight from employers.",
      icon: Briefcase,
    },
    {
      key: "news",
      label: "Tech news",
      blurb: "Hacker News, Lobsters, TLDR and a handful of others.",
      icon: Newspaper,
    },
    {
      key: "events",
      label: "Events & hackathons",
      blurb: "Meetups and hackathons, with their deadlines.",
      icon: Calendar,
    },
  ];

  const TITLE_ID = "setup-title";
  const LAST_QUESTION = 2;
  const SCRAPE_STEP = 3;

  const firstRun = $derived(feed.onboarding === "needed");
  const offered = $derived(feed.onboarding === "offered");
  /** Raised again from Settings by someone who answered it long ago. */
  const reopened = $derived(feed.setupOpen && feed.onboarding === "done");

  // Prefilled from what is already stored, so re-opening it edits rather than
  // starts over, and an existing install sees its own interests ticked.
  let step = $state(0);
  let phase = $state<"asking" | "scraping" | "done" | "failed">("asking");
  let failure = $state<string | null>(null);
  let found = $state<number | null>(null);
  let wanted = $state({
    internships: feed.prefs.internships,
    jobs: feed.prefs.jobs,
    news: feed.prefs.news,
    events: feed.prefs.events,
  });
  let interests = $state<string[]>([...feed.interests]);
  let region = $state<string | null>(feed.homeRegion);
  let levels = $state<string[]>([...feed.targetSeniority]);
  let heading = $state<HTMLElement | null>(null);

  const anyKind = $derived(wanted.internships || wanted.jobs || wanted.news || wanted.events);
  const wantsRoles = $derived(wanted.internships || wanted.jobs);
  const scraping = $derived(phase === "scraping");

  // A new step is a new screen: take focus to its heading so a keyboard or a
  // screen reader lands at the top of it rather than on a button that has gone.
  $effect(() => {
    void step;
    void phase;
    heading?.focus();
  });

  function toggle(list: string[], value: string): string[] {
    return list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
  }

  /** What the scrape is about to run, so the list can show it before it moves. */
  const rows = $derived(
    (feed.status?.sources ?? [])
      .filter((s) => s.enabled)
      .map((source) => ({ source, result: feed.progress.find((p) => p.source === source.name) })),
  );
  const finished = $derived(rows.filter((r) => r.result).length);

  async function start() {
    step = SCRAPE_STEP;
    phase = "scraping";
    failure = null;
    try {
      // Anything the reader switched off one source at a time in Settings
      // earlier is kept: the form decides kinds, not individual sources.
      await feed.saveSetup(
        { ...wanted, disabled_sources: feed.prefs.disabled_sources },
        interests,
        region,
        levels,
      );
      found = await feed.scrapeNow();
      phase = "done";
    } catch (e) {
      failure = String(e);
      phase = "failed";
    }
  }

  function back() {
    if (phase === "failed") {
      phase = "asking";
      step = LAST_QUESTION;
    } else if (step > 0) {
      step -= 1;
    }
  }

  function dismiss() {
    if (reopened) feed.closeSetup();
    else if (offered) void feed.skipSetup().catch((e) => (failure = String(e)));
  }

  function onKeydown(e: KeyboardEvent) {
    // Only where leaving costs nothing: never mid-scrape, never on a first run.
    if (e.key === "Escape" && !scraping && !firstRun) dismiss();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  role="dialog"
  aria-modal="true"
  aria-labelledby={TITLE_ID}
  class="fixed inset-0 z-[100] overflow-y-auto bg-background"
>
  <div class="mx-auto flex min-h-full w-full max-w-2xl flex-col px-5 py-8 sm:px-6 sm:py-12">
    <!-- Header -->
    <div class="mb-6 flex items-center justify-between gap-3">
      <span class="flex items-center gap-2 font-display text-lg font-bold tracking-tight">
        <Sparkles size={18} class="text-brand" />
        Anti-FOMO
      </span>
      {#if (offered || reopened) && !scraping}
        <button
          onclick={dismiss}
          class="inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium text-subtle transition-colors hover:bg-line-soft hover:text-foreground"
        >
          {reopened ? "Close" : "Skip for now"}
          <X size={14} />
        </button>
      {/if}
    </div>

    <!-- Progress rail -->
    <div class="mb-8 flex items-center gap-1.5" aria-hidden="true">
      {#each [0, 1, 2, 3] as i (i)}
        <span
          class="h-1 flex-1 rounded-full transition-colors {i <= step ? 'bg-brand' : 'bg-line-soft'}"
        ></span>
      {/each}
    </div>

    <div class="flex-1">
      {#if step === 0}
        <h1
          id={TITLE_ID}
          bind:this={heading}
          tabindex="-1"
          class="mb-2 font-display text-2xl font-bold tracking-tight outline-none sm:text-3xl"
        >
          {firstRun ? "What do you want to keep up with?" : "What should we collect?"}
        </h1>
        <p class="mb-6 text-[15px] leading-relaxed text-muted">
          Nothing is fetched until you answer. Whatever you leave off is never contacted at all,
          which keeps this fast and keeps your feed to what you asked for.
        </p>
        <div class="grid gap-3 sm:grid-cols-2">
          {#each KINDS as kind (kind.key)}
            {@const on = wanted[kind.key]}
            {@const Icon = kind.icon}
            <button
              type="button"
              aria-pressed={on}
              onclick={() => (wanted[kind.key] = !on)}
              class="card flex items-start gap-3 border p-4 text-left transition-colors {on
                ? 'border-brand bg-brand-soft'
                : 'border-line hover:border-brand'}"
            >
              <span
                class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl {on
                  ? 'bg-brand text-brand-fg'
                  : 'bg-line-soft text-subtle'}"
              >
                <Icon size={18} strokeWidth={2.25} />
              </span>
              <span class="min-w-0">
                <span class="block text-sm font-semibold {on ? 'text-brand-soft-fg' : ''}"
                  >{kind.label}</span
                >
                <span class="mt-0.5 block text-xs leading-snug text-muted">{kind.blurb}</span>
              </span>
            </button>
          {/each}
        </div>
        {#if !anyKind}
          <p class="mt-4 text-sm text-star" role="status">Pick at least one to continue.</p>
        {/if}
      {:else if step === 1}
        <h1
          id={TITLE_ID}
          bind:this={heading}
          tabindex="-1"
          class="mb-2 font-display text-2xl font-bold tracking-tight outline-none sm:text-3xl"
        >
          What are you interested in?
        </h1>
        <p class="mb-6 text-[15px] leading-relaxed text-muted">
          These decide what rises to the top, and which postings count as a really good match when
          a deadline is close. Pick as many as you like, or none.
        </p>
        <div class="flex flex-wrap gap-2">
          {#each feed.availableInterests as name (name)}
            {@const on = interests.includes(name)}
            <button
              type="button"
              aria-pressed={on}
              onclick={() => (interests = toggle(interests, name))}
              class="chip px-4 py-2 text-sm {on
                ? 'bg-brand text-brand-fg'
                : 'border border-line text-muted hover:border-brand'}"
            >
              {name}
            </button>
          {/each}
        </div>
      {:else if step === 2}
        <h1
          id={TITLE_ID}
          bind:this={heading}
          tabindex="-1"
          class="mb-2 font-display text-2xl font-bold tracking-tight outline-none sm:text-3xl"
        >
          {wantsRoles ? "Where, and at what level?" : "Where are you?"}
        </h1>
        <p class="mb-6 text-[15px] leading-relaxed text-muted">
          Roles and events near you rank higher. Both are optional.
        </p>

        <p class="mb-2 flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase">
          <MapPin size={12} /> Where
        </p>
        <div class="mb-7 flex flex-wrap gap-2">
          <button
            type="button"
            aria-pressed={region === null}
            onclick={() => (region = null)}
            class="chip px-4 py-2 text-sm {region === null
              ? 'bg-brand text-brand-fg'
              : 'border border-line text-muted hover:border-brand'}"
          >
            Anywhere
          </button>
          {#each feed.setupOptions.regions as place (place)}
            {@const on = region === place}
            <button
              type="button"
              aria-pressed={on}
              onclick={() => (region = place)}
              class="chip px-4 py-2 text-sm {on
                ? 'bg-brand text-brand-fg'
                : 'border border-line text-muted hover:border-brand'}"
            >
              {place}
            </button>
          {/each}
        </div>

        {#if wantsRoles}
          <p
            class="mb-2 flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase"
          >
            <GraduationCap size={12} /> Level
          </p>
          <div class="flex flex-wrap gap-2">
            {#each feed.setupOptions.seniority as level (level)}
              {@const on = levels.includes(level)}
              <button
                type="button"
                aria-pressed={on}
                onclick={() => (levels = toggle(levels, level))}
                class="chip px-4 py-2 text-sm {on
                  ? 'bg-brand text-brand-fg'
                  : 'border border-line text-muted hover:border-brand'}"
              >
                {level}
              </button>
            {/each}
          </div>
        {/if}
      {:else}
        <h1
          id={TITLE_ID}
          bind:this={heading}
          tabindex="-1"
          class="mb-2 font-display text-2xl font-bold tracking-tight outline-none sm:text-3xl"
        >
          {#if phase === "done"}
            You're all set
          {:else if phase === "failed"}
            That didn't work
          {:else}
            Collecting your feed…
          {/if}
        </h1>
        <p class="mb-5 text-[15px] leading-relaxed text-muted" aria-live="polite">
          {#if phase === "done"}
            {found === null
              ? "Your feed is ready."
              : `${found.toLocaleString()} items collected.`}
            Everything is stored on this device.
          {:else if phase === "failed"}
            {failure}
          {:else}
            {finished} of {rows.length} sources done. Employer boards are the slow ones — the feed
            fills in as they finish.
          {/if}
        </p>

        <div
          class="mb-4 h-1.5 overflow-hidden rounded-full bg-line-soft"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={rows.length}
          aria-valuenow={finished}
          aria-label="Sources finished"
        >
          <div
            class="h-full rounded-full bg-brand transition-[width] duration-300"
            style:width="{rows.length ? (finished / rows.length) * 100 : 0}%"
          ></div>
        </div>

        <ul class="flex flex-col divide-y divide-line-soft rounded-xl border border-line-soft">
          {#each rows as { source, result } (source.name)}
            <li class="flex items-center justify-between gap-3 px-3.5 py-2.5 text-sm">
              <span class="flex min-w-0 items-center gap-2.5">
                {#if !result}
                  {#if scraping}
                    <LoaderCircle size={15} class="shrink-0 animate-spin text-subtle" />
                  {:else}
                    <Minus size={15} class="shrink-0 text-subtle" />
                  {/if}
                {:else if result.status === "failed"}
                  <CircleAlert size={15} class="shrink-0 text-star" />
                {:else if result.status === "dormant"}
                  <Minus size={15} class="shrink-0 text-subtle" />
                {:else}
                  <CircleCheck size={15} class="shrink-0 text-job" />
                {/if}
                <span class="truncate font-medium">{source.name}</span>
              </span>
              <span class="shrink-0 truncate text-xs text-subtle">
                {#if !result}
                  {scraping ? "working…" : ""}
                {:else if result.status === "failed"}
                  <span class="text-star">didn't answer</span>
                {:else if result.status === "dormant"}
                  no longer updated
                {:else}
                  {result.count.toLocaleString()} found
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Footer -->
    <div class="mt-8 flex items-center justify-between gap-3">
      {#if (step > 0 && !scraping && phase !== "done") || phase === "failed"}
        <button
          type="button"
          onclick={back}
          class="inline-flex items-center gap-1.5 rounded-xl px-4 py-2.5 text-sm font-semibold text-muted transition-colors hover:bg-line-soft hover:text-foreground"
        >
          <ArrowLeft size={15} /> Back
        </button>
      {:else}
        <span></span>
      {/if}

      {#if step < LAST_QUESTION}
        <button
          type="button"
          onclick={() => (step += 1)}
          disabled={step === 0 && !anyKind}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
        >
          Continue <ChevronRight size={15} />
        </button>
      {:else if step === LAST_QUESTION}
        <button
          type="button"
          onclick={start}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
        >
          {reopened ? "Save and refresh" : "Start collecting"} <ChevronRight size={15} />
        </button>
      {:else if phase === "done"}
        <button
          type="button"
          onclick={() => feed.finishSetup()}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
        >
          {reopened ? "Done" : "Open Today"} <ChevronRight size={15} />
        </button>
      {:else if phase === "failed"}
        <button
          type="button"
          onclick={start}
          class="inline-flex items-center gap-1.5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
        >
          Try again
        </button>
      {/if}
    </div>
  </div>
</div>
