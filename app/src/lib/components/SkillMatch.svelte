<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { skillCategoryIcon } from "$lib/icons";
  import { groupByCategory } from "$lib/skills";
  import SkillChip from "./SkillChip.svelte";

  // What this posting wants, and how much of it the user has. The chips are
  // live: tapping one edits the profile, which is the point — a skills list
  // built while reading jobs stays truer than one filled in once and
  // forgotten.
  //
  // Every skill here was read out of the employer's own words: the
  // requirements, the duties and the description the enrichment pass fetched,
  // plus anything the source itself tagged the posting with. Nothing is
  // inferred from the job title, so the heading is unconditional — there is no
  // second case for it to have to hedge about.
  let { item }: { item: ScrapedItem } = $props();

  /** Segments in the meter. Fixed, so the bar reads the same on every job. */
  const SEGMENTS = 10;

  const required = $derived(item.required_skills ?? []);
  // Counted against the live profile rather than the `matched_skills` the
  // payload froze, so tapping a chip below moves the meter on the tap.
  const have = $derived(required.reduce((n, s) => n + (feed.hasSkill(s) ? 1 : 0), 0));
  const groups = $derived(groupByCategory(required, feed.skillCatalog));
  const filled = $derived(
    required.length === 0 ? 0 : Math.round((have / required.length) * SEGMENTS),
  );
  const summary = $derived(`${have} of ${required.length} skills`);
</script>

<div class="mb-4 rounded-xl bg-line-soft p-4">
  <div class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1.5">
    <p class="text-xs font-bold tracking-wide text-subtle uppercase">Skills this posting asks for</p>
    <!-- The label carries the figure, so this is one image to a screen reader
         rather than ten meaningless segments. -->
    <div class="flex items-center gap-0.5" role="img" aria-label={summary}>
      {#each { length: SEGMENTS } as _, i (i)}
        <span
          class="h-1.5 w-3 rounded-full transition-colors {i < filled ? 'bg-brand' : 'bg-surface'}"
        ></span>
      {/each}
    </div>
    <p class="text-xs font-bold text-foreground">{summary}</p>
  </div>

  {#each groups as group (group.name)}
    {@const Icon = skillCategoryIcon(group.name)}
    <div class="mb-3 last:mb-0">
      <p class="mb-1.5 flex items-center gap-1.5 text-[11px] font-bold tracking-wide text-subtle uppercase">
        <Icon size={11} />
        {group.name}
      </p>
      <div class="flex flex-wrap gap-1.5">
        {#each group.skills as skill (skill)}
          <SkillChip
            {skill}
            size="sm"
            has={feed.hasSkill(skill)}
            onToggle={(s) => void feed.toggleSkill(s)}
          />
        {/each}
      </div>
    </div>
  {/each}

  <div class="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-line pt-2.5">
    <p class="text-[11px] text-subtle">
      Read from this posting's own text. Tap a skill to say whether you have it.
    </p>
    <button
      onclick={() => feed.openSkillsForm()}
      class="text-[11px] font-semibold text-brand hover:underline"
    >
      Edit all my skills
    </button>
  </div>
</div>
