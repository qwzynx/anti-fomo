<script lang="ts">
  import type { IconComponent } from "$lib/icons";

  // A band heading. The redesign separates content by type using full-width
  // bands and hairlines rather than by putting each type in its own box, so
  // this is the only chrome a section gets: a coloured glyph, its name, a line
  // saying how it is ordered, and one way out to the rest of it.
  let {
    icon,
    label,
    blurb,
    accent = "text-brand",
    actionLabel,
    href,
    onAction,
    children,
  }: {
    icon: IconComponent;
    label: string;
    /** Why these items and in this order — the part a heading alone leaves out. */
    blurb?: string;
    accent?: string;
    actionLabel?: string;
    /** A link when the rest lives elsewhere, a button when it expands in place. */
    href?: string;
    onAction?: () => void;
    children: import("svelte").Snippet;
  } = $props();

  const Icon = $derived(icon);
</script>

<section>
  <div class="flex items-baseline justify-between gap-3 pt-4 pb-2">
    <h2 class="flex min-w-0 items-center gap-2.5">
      <Icon size={15} strokeWidth={2.5} class="shrink-0 {accent}" />
      <span class="font-display text-[17px] font-bold tracking-tight">{label}</span>
      {#if blurb}
        <span class="hidden truncate text-xs font-normal text-subtle sm:inline">{blurb}</span>
      {/if}
    </h2>

    {#if actionLabel}
      {#if href}
        <a {href} class="shrink-0 text-[13px] font-semibold text-brand hover:underline">
          {actionLabel}
        </a>
      {:else if onAction}
        <button
          onclick={onAction}
          class="shrink-0 text-[13px] font-semibold text-brand hover:underline"
        >
          {actionLabel}
        </button>
      {/if}
    {/if}
  </div>

  {@render children()}
</section>
