<script lang="ts">
  import type { ScrapedItem } from "$lib/api";
  import { ChevronDown, Gift, ListChecks, Target } from "$lib/icons";

  // The posting as the employer wrote it, once the enrichment pass has fetched
  // it. Most sources give us nothing, so this renders only what it has and the
  // caller keeps its placeholder for the rest.
  //
  // Requirements and Responsibilities open by default because they are what
  // someone deciding whether to apply actually reads; Perks is a nice-to-know
  // and stays folded so the pane does not become a wall of text.
  let { item }: { item: ScrapedItem } = $props();

  type Section = {
    key: string;
    label: string;
    icon: typeof ListChecks;
    lines: string[];
    open: boolean;
  };

  /** The stored fields are newline-joined bullets; split them back apart. */
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
        { key: "req", label: "Requirements", icon: ListChecks, lines: lines(item.requirements), open: true },
        { key: "resp", label: "Responsibilities", icon: Target, lines: lines(item.responsibilities), open: true },
        { key: "perks", label: "Perks & benefits", icon: Gift, lines: lines(item.perks), open: false },
      ] satisfies Section[]
    ).filter((s) => s.lines.length > 0),
  );

  // Only worth showing when it adds something the sections do not.
  const overview = $derived(sections.length === 0 ? (item.description ?? "").trim() : "");
</script>

{#if sections.length > 0 || overview}
  <div class="mb-4 flex flex-col gap-2">
    {#each sections as section (section.key)}
      {@const Icon = section.icon}
      <details open={section.open} class="group rounded-xl bg-line-soft p-4">
        <summary
          class="flex cursor-pointer list-none items-center gap-1.5 text-xs font-bold tracking-wide text-subtle uppercase [&::-webkit-details-marker]:hidden"
        >
          <Icon size={12} />
          {section.label}
          <span class="font-semibold normal-case">· {section.lines.length}</span>
          <ChevronDown
            size={14}
            class="ml-auto shrink-0 transition-transform group-open:rotate-180"
          />
        </summary>
        <ul class="mt-2.5 flex list-none flex-col gap-1.5">
          {#each section.lines as line (line)}
            <li class="flex gap-2 text-sm leading-relaxed text-muted">
              <span class="mt-2 h-1 w-1 shrink-0 rounded-full bg-subtle" aria-hidden="true"></span>
              {line}
            </li>
          {/each}
        </ul>
      </details>
    {/each}

    {#if overview}
      <div class="rounded-xl bg-line-soft p-4">
        <p class="mb-2 text-xs font-bold tracking-wide text-subtle uppercase">About the role</p>
        <p class="text-sm leading-relaxed whitespace-pre-line text-muted">{overview}</p>
      </div>
    {/if}
  </div>
{/if}
