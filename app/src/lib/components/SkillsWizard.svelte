<script lang="ts">
  import { feed } from "$lib/feed.svelte";
  import {
    ArrowLeft,
    Check,
    ChevronRight,
    CircleCheck,
    Search,
    Sparkles,
    X,
    skillCategoryIcon,
  } from "$lib/icons";
  import { groupByCategory, stepsFor } from "$lib/skills";
  import Modal from "./Modal.svelte";
  import SkillChip from "./SkillChip.svelte";

  // The skills form, in its two lives. The first time it opens it is a paced
  // wizard — one theme per step, an intro and a review — because a wall of a
  // hundred and ten chips is not something anyone fills in cold. Every time
  // after, it is a flat editor: the user already knows the shape of the list
  // and wants to change three things and leave.
  //
  // Which one you get is decided by `skills_setup_at`, not by whether the
  // skill list is empty, so finishing the wizard having picked nothing still
  // counts as having done it.
  //
  // Mounted once from the root layout; opened through the store so the feed
  // banner, the settings button and a job detail can all raise it.

  const TITLE_ID = "skills-form-title";

  const catalog = $derived(feed.skillCatalog);
  const steps = $derived(stepsFor(catalog));
  /** Intro is 0, the category steps are 1..n, and the review is last. */
  const lastStep = $derived(steps.length + 1);

  let step = $state(0);
  let query = $state("");

  const editing = $derived(!feed.needsSkillsSetup);
  const chosen = $derived(new Set(feed.skills));
  const total = $derived(feed.skills.length);

  // A fresh open starts at the beginning and drops the previous search.
  $effect(() => {
    if (feed.skillsFormOpen) {
      step = 0;
      query = "";
    }
  });

  /** Catalog filtered by the editor's search box. Empty query means all of it. */
  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return catalog;
    return catalog
      .map((category) => ({
        name: category.name,
        skills: category.skills.filter((s) => s.toLowerCase().includes(needle)),
      }))
      .filter((category) => category.skills.length > 0);
  });

  /** What the user picked, grouped for the review step. */
  const picked = $derived(groupByCategory(feed.skills, catalog));

  function toggle(skill: string) {
    void feed.toggleSkill(skill);
  }

  function countIn(skills: string[]) {
    return skills.filter((s) => chosen.has(s)).length;
  }

  async function finish() {
    await feed.completeSkillsSetup();
    feed.closeSkillsForm();
  }

  function close() {
    feed.closeSkillsForm();
  }
</script>

<!-- One category and its chips. Shared by both modes so they cannot drift. -->
{#snippet categoryBlock(category: { name: string; skills: string[] })}
  {@const Icon = skillCategoryIcon(category.name)}
  {@const have = countIn(category.skills)}
  <section class="mb-6 last:mb-0">
    <div class="mb-2.5 flex items-center gap-2">
      <Icon size={15} class="text-brand" />
      <h3 class="text-xs font-bold tracking-wide text-subtle uppercase">{category.name}</h3>
      {#if have > 0}
        <span class="rounded-md bg-brand-soft px-1.5 py-0.5 text-[11px] font-bold text-brand-soft-fg">
          {have}
        </span>
      {/if}
    </div>
    <div class="flex flex-wrap gap-2">
      {#each category.skills as skill (skill)}
        <SkillChip {skill} has={chosen.has(skill)} onToggle={toggle} />
      {/each}
    </div>
  </section>
{/snippet}

<Modal open={feed.skillsFormOpen} onClose={close} titleId={TITLE_ID} size="lg">
  <div class="flex min-h-0 flex-1 flex-col">
    <!-- Header -->
    <div class="flex items-start justify-between gap-4 border-b border-line-soft p-5 sm:p-6">
      <div class="min-w-0">
        <span
          class="inline-flex items-center gap-1 rounded-full bg-brand-soft px-2 py-0.5 text-[10px] font-bold tracking-wider text-brand-soft-fg uppercase"
        >
          <Sparkles size={11} strokeWidth={2.5} />
          {editing ? "Your skills" : "Setting up"}
        </span>
        <h2 id={TITLE_ID} class="mt-1.5 text-xl leading-tight font-bold">
          {#if editing}
            Edit your skills
          {:else if step === 0}
            What can you build?
          {:else if step === lastStep}
            That's the picture
          {:else}
            {steps[step - 1].title}
          {/if}
        </h2>
        <p class="text-sm text-muted">
          {#if editing}
            {total} selected across {picked.length} categories
          {:else if step === 0}
            Two minutes now, and every job tells you how well you match.
          {:else if step === lastStep}
            {total} skills across {picked.length} categories.
          {:else}
            {steps[step - 1].blurb}
          {/if}
        </p>
      </div>
      <button
        onclick={close}
        aria-label="Close"
        class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-subtle transition-colors hover:bg-line-soft hover:text-foreground"
      >
        <X size={18} />
      </button>
    </div>

    <!-- Progress rail, wizard only -->
    {#if !editing}
      <div class="flex shrink-0 items-center gap-1.5 px-5 pt-4 sm:px-6" aria-hidden="true">
        {#each { length: lastStep + 1 } as _, i (i)}
          <span
            class="h-1 flex-1 rounded-full transition-colors {i <= step
              ? 'bg-brand'
              : 'bg-line-soft'}"
          ></span>
        {/each}
      </div>
    {/if}

    <!-- Search, editor only -->
    {#if editing}
      <div class="shrink-0 px-5 pt-4 sm:px-6">
        <div class="relative">
          <Search
            size={15}
            class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-subtle"
          />
          <input
            bind:value={query}
            type="search"
            placeholder="Search skills…"
            aria-label="Search skills"
            class="control control-focus w-full pl-9"
          />
        </div>
      </div>
    {/if}

    <!-- Body -->
    <div class="min-h-0 flex-1 overflow-y-auto p-5 sm:p-6">
      {#if editing}
        {#each filtered as category (category.name)}
          {@render categoryBlock(category)}
        {/each}
        {#if filtered.length === 0}
          <p class="py-8 text-center text-sm text-muted">
            Nothing matches “{query}”.
          </p>
        {/if}
      {:else if step === 0}
        <div class="py-2">
          <p class="mb-5 text-[15px] leading-relaxed text-muted">
            Tell us what you already know how to do. Every job gets matched against the
            skills its own posting asks for, so you'll see at a glance how much of a role
            you already cover — and postings you fit move up the list.
          </p>
          <ul class="flex flex-col gap-3">
            {#each ["Tap the skills you have. Skip anything you don't.", "You can change any of it later, from Settings or from any job.", "Nothing leaves your machine — this is all stored locally."] as line (line)}
              <li class="flex items-start gap-2.5 text-sm text-muted">
                <CircleCheck size={16} class="mt-0.5 shrink-0 text-brand" />
                {line}
              </li>
            {/each}
          </ul>
        </div>
      {:else if step === lastStep}
        {#if picked.length === 0}
          <p class="py-8 text-center text-sm text-muted">
            You didn't pick any skills. That's fine — jobs will still show what they're
            asking for, and you can tick them off as you read.
          </p>
        {:else}
          {#each picked as category (category.name)}
            {@const Icon = skillCategoryIcon(category.name)}
            <section class="mb-5 last:mb-0 rounded-xl bg-line-soft p-4">
              <p
                class="mb-2 flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase"
              >
                <Icon size={12} />
                {category.name}
                <span class="font-semibold normal-case">· {category.skills.length}</span>
              </p>
              <div class="flex flex-wrap gap-1.5">
                {#each category.skills as skill (skill)}
                  <span
                    class="inline-flex items-center gap-1 rounded-md bg-surface px-2 py-0.5 text-[11px] font-semibold text-muted shadow-sm"
                  >
                    <Check size={11} strokeWidth={3} class="text-brand" />
                    {skill}
                  </span>
                {/each}
              </div>
            </section>
          {/each}
        {/if}
      {:else}
        {#each steps[step - 1].groups as category (category.name)}
          {@render categoryBlock(category)}
        {/each}
      {/if}
    </div>

    <!-- Footer -->
    <div class="shrink-0 border-t border-line-soft p-4">
      {#if editing}
        <div class="flex items-center justify-between gap-3">
          <button
            onclick={() => feed.setSkills([])}
            disabled={total === 0}
            class="text-xs font-semibold text-brand disabled:cursor-not-allowed disabled:text-subtle"
          >
            Clear all
          </button>
          <button
            onclick={close}
            class="rounded-xl bg-brand px-6 py-2.5 font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
          >
            Done
          </button>
        </div>
      {:else}
        <div class="flex items-center justify-between gap-3">
          {#if step > 0}
            <button
              onclick={() => (step -= 1)}
              class="flex items-center gap-1 rounded-xl px-3 py-2.5 text-sm font-semibold text-muted transition-colors hover:bg-line-soft hover:text-foreground"
            >
              <ArrowLeft size={15} />
              Back
            </button>
          {:else}
            <span></span>
          {/if}

          <div class="flex items-center gap-2">
            {#if step > 0 && step < lastStep}
              <button
                onclick={() => (step = lastStep)}
                class="rounded-xl px-3 py-2.5 text-sm font-semibold text-subtle transition-colors hover:text-foreground"
              >
                Skip the rest
              </button>
            {/if}
            {#if step === lastStep}
              <button
                onclick={finish}
                class="flex items-center gap-1.5 rounded-xl bg-brand px-6 py-2.5 font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
              >
                Save my skills
                <Check size={16} strokeWidth={2.5} />
              </button>
            {:else}
              <button
                onclick={() => (step += 1)}
                class="flex items-center gap-1 rounded-xl bg-brand px-6 py-2.5 font-semibold text-brand-fg transition-all hover:bg-brand-hover hover:shadow-lg hover:shadow-brand/25"
              >
                {step === 0 ? "Get started" : "Next"}
                <ChevronRight size={16} strokeWidth={2.5} />
              </button>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
</Modal>
