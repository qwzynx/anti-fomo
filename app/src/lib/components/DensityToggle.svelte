<script lang="ts">
  import { DENSITIES, density } from "$lib/density.svelte";
</script>

<div
  role="group"
  aria-label="Result density"
  class="flex shrink-0 items-center gap-0.5 rounded-xl border border-line bg-surface p-0.5"
>
  {#each DENSITIES as option (option.value)}
    {@const Icon = option.icon}
    {@const active = density.value === option.value}
    <button
      onclick={() => density.set(option.value)}
      aria-pressed={active}
      title="{option.label} view"
      aria-label="{option.label} view"
      class="flex h-8 w-8 items-center justify-center rounded-lg transition-colors
             {active ? 'bg-brand text-brand-fg' : 'text-subtle hover:bg-line-soft hover:text-foreground'}"
    >
      <Icon size={15} strokeWidth={active ? 2.5 : 2} />
    </button>
  {/each}
</div>
