<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { BookOpen, ChevronDown, Gift, ListChecks, Target } from "$lib/icons";

  // The posting as the employer wrote it, once the enrichment pass has fetched
  // it. Every field is optional, so this renders what it has and the caller
  // keeps its placeholder for the postings that came back with nothing.
  //
  // Requirements and Responsibilities open by default because they are what
  // someone deciding whether to apply actually reads. Perks and the overview
  // stay folded: the first is a nice-to-know, and the second is where the
  // company boilerplate lives, so unfolding both would bury the two sections
  // that matter under a wall of text.
  let { item }: { item: ScrapedItem } = $props();

  type Section = {
    key: string;
    label: string;
    icon: typeof ListChecks;
    lines: string[];
    open: boolean;
    /** Paragraphs rather than a bulleted list. */
    prose?: boolean;
  };

  /** The stored fields are newline-joined lines; split them back apart. */
  function lines(text: string | null | undefined): string[] {
    if (!text) return [];
    return text
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);
  }

  const overview = $derived(lines(item.description));

  const sections = $derived(
    (
      [
        { key: "req", label: "Requirements", icon: ListChecks, lines: lines(item.requirements), open: true },
        { key: "resp", label: "Responsibilities", icon: Target, lines: lines(item.responsibilities), open: true },
        { key: "perks", label: "Perks & benefits", icon: Gift, lines: lines(item.perks), open: false },
        // Last and folded — unless it is the only thing the page gave us, in
        // which case a collapsed panel would hide the whole posting.
        {
          key: "about",
          label: "About the role",
          icon: BookOpen,
          lines: overview,
          open: !item.requirements && !item.responsibilities && !item.perks,
          prose: true,
        },
      ] satisfies Section[]
    ).filter((s) => s.lines.length > 0),
  );
</script>

{#if sections.length > 0}
  <div class="mb-4 flex flex-col gap-2">
    {#each sections as section (section.key)}
      {@const Icon = section.icon}
      <details open={section.open} class="group rounded-xl bg-line-soft p-4">
        <summary
          class="flex cursor-pointer list-none items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase [&::-webkit-details-marker]:hidden"
        >
          <Icon size={12} />
          {section.label}
          {#if !section.prose}
            <span class="font-semibold normal-case">· {section.lines.length}</span>
          {/if}
          <ChevronDown
            size={14}
            class="ml-auto shrink-0 transition-transform group-open:rotate-180"
          />
        </summary>
        {#if section.prose}
          <div class="mt-2.5 flex flex-col gap-2">
            {#each section.lines as line (line)}
              <p class="text-sm leading-relaxed text-muted">{line}</p>
            {/each}
          </div>
        {:else}
          <ul class="mt-2.5 flex list-none flex-col gap-1.5">
            {#each section.lines as line (line)}
              <li class="flex gap-2 text-sm leading-relaxed text-muted">
                <span class="mt-2 h-1 w-1 shrink-0 rounded-full bg-subtle" aria-hidden="true"></span>
                {line}
              </li>
            {/each}
          </ul>
        {/if}
      </details>
    {/each}
  </div>
{/if}
