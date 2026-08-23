<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { BookOpen, Gift, ListChecks, Target } from "$lib/icons";

  // The posting as the employer wrote it, once the enrichment pass has fetched
  // it. Every field is optional, so this renders what it has and the caller
  // keeps its placeholder for the postings that came back with nothing.
  //
  // Nothing here is folded away by default. This *is* the posting — it is what
  // the reader opened the item to read — and a collapsed panel labelled "About
  // the role" hid the whole description behind a click nobody knew to make.
  // Perks stay last because they are the nice-to-know, not because they are
  // worth less than a fold.
  let { item }: { item: ScrapedItem } = $props();

  type Section = {
    key: string;
    label: string;
    icon: typeof ListChecks;
    lines: string[];
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

  const sections = $derived(
    (
      [
        {
          key: "about",
          label: "About the role",
          icon: BookOpen,
          lines: lines(item.description),
          prose: true,
        },
        { key: "req", label: "Requirements", icon: ListChecks, lines: lines(item.requirements) },
        {
          key: "resp",
          label: "Responsibilities",
          icon: Target,
          lines: lines(item.responsibilities),
        },
        { key: "perks", label: "Perks & benefits", icon: Gift, lines: lines(item.perks) },
      ] satisfies Section[]
    ).filter((s) => s.lines.length > 0),
  );
</script>

{#if sections.length > 0}
  <div class="mb-4 flex flex-col gap-2">
    {#each sections as section (section.key)}
      {@const Icon = section.icon}
      <section class="rounded-xl bg-line-soft p-4">
        <h3
          class="flex items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase"
        >
          <Icon size={12} />
          {section.label}
          {#if !section.prose}
            <span class="font-semibold normal-case">· {section.lines.length}</span>
          {/if}
        </h3>
        {#if section.prose}
          <div class="mt-2.5 flex flex-col gap-2">
            {#each section.lines as line, i (i)}
              <p class="text-sm leading-relaxed text-muted">{line}</p>
            {/each}
          </div>
        {:else}
          <ul class="mt-2.5 flex list-none flex-col gap-1.5">
            {#each section.lines as line, i (i)}
              <li class="flex gap-2 text-sm leading-relaxed text-muted">
                <span class="mt-2 h-1 w-1 shrink-0 rounded-full bg-subtle" aria-hidden="true"></span>
                {line}
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}
  </div>
{/if}
