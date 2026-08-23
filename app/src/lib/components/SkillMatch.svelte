<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { skillCategoryIcon } from "$lib/icons";
  import { groupByCategory } from "$lib/skills";
  import SkillChip from "./SkillChip.svelte";

  // What this role wants, and how much of it the user has. The chips are
  // live: tapping one edits the profile, which is the point — a skills list
  // built while reading jobs stays truer than one filled in once and
  // forgotten.
  //
  // The heading follows the evidence. When the enrichment pass reached this
  // posting these skills were read out of its own requirements, so it says so;
  // otherwise they came from `skills::ROLES` inferring from the job title, and
  // claiming the posting "asked for" them would overstate what we know.
  let { item }: { item: ScrapedItem } = $props();

  /** Segments in the meter. Fixed, so the bar reads the same on every job. */
  const SEGMENTS = 10;

  const required = $derived(item.required_skills ?? []);
  const have = $derived(item.matched_skills?.length ?? 0);
  const groups = $derived(groupByCategory(required, feed.skillCatalog));
  const filled = $derived(
    required.length === 0 ? 0 : Math.round((have / required.length) * SEGMENTS),
  );
  const summary = $derived(`${have} of ${required.length} skills`);
  const fromPosting = $derived(Boolean(item.requirements || item.responsibilities));
</script>

<div class="mb-4 rounded-xl bg-line-soft p-4">
  <div class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1.5">
    <p class="text-xs font-bold tracking-wide text-subtle uppercase">
      {fromPosting ? "Skills this posting asks for" : "Typically wanted for this role"}
    </p>
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
            has={item.matched_skills?.includes(skill) ?? false}
            onToggle={(s) => void feed.toggleSkill(s)}
          />
        {/each}
      </div>
    </div>
  {/each}

  <div class="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-line pt-2.5">
    <p class="text-[11px] text-subtle">
      Tap a skill to say whether you have it.{fromPosting
        ? ""
        : " Inferred from the role — check the posting."}
    </p>
    <button
      onclick={() => feed.openSkillsForm()}
      class="text-[11px] font-semibold text-brand hover:underline"
    >
      Edit all my skills
    </button>
  </div>
</div>
