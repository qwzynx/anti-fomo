<script lang="ts">
  import { HUB_SORT_BLURBS, HUB_SORTS, type HubSort } from "$lib/filters";
  import {
    ArrowUpDown,
    Banknote,
    Building2,
    Check,
    Clock,
    Hourglass,
    Sparkles,
    type IconComponent,
  } from "$lib/icons";

  // A chip-styled trigger opening a popover of named sorts, rather than a
  // native <select>: the same "everything is a chip" language as the filter
  // bar it sits beside, and a select can't carry a second line explaining
  // what each order actually means.
  let { value = $bindable() }: { value: HubSort } = $props();

  let open = $state(false);

  function iconFor(sort: HubSort): IconComponent {
    switch (sort) {
      case "Newest posted":
        return Clock;
      case "Closing soonest":
      case "Closing latest":
        return Hourglass;
      case "Highest pay":
        return Banknote;
      case "Company A–Z":
        return Building2;
      default:
        return Sparkles;
    }
  }

  function pick(sort: HubSort) {
    value = sort;
    open = false;
  }
</script>

<svelte:window onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="relative shrink-0">
  <button
    onclick={() => (open = !open)}
    aria-haspopup="listbox"
    aria-expanded={open}
    class="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-full border px-3 text-[13px] font-semibold transition-colors
           {open
      ? 'border-brand bg-brand-soft text-brand-soft-fg'
      : 'border-line bg-surface text-muted hover:border-subtle'}"
  >
    <ArrowUpDown size={13} strokeWidth={2.25} class="shrink-0" />
    {value}
  </button>

  {#if open}
    <!-- Catches an outside click without a dark scrim — this is a menu, not a
         modal, and the page underneath should keep reading normally. -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="fixed inset-0 z-40" onclick={() => (open = false)}></div>

    <div
      role="listbox"
      aria-label="Sort roles by"
      class="animate-fade-up card absolute right-0 z-50 mt-2 w-72 origin-top-right overflow-hidden bg-elevated p-1.5 shadow-2xl"
    >
      {#each HUB_SORTS as sort (sort)}
        {@const Icon = iconFor(sort)}
        {@const active = sort === value}
        <button
          role="option"
          aria-selected={active}
          onclick={() => pick(sort)}
          class="flex w-full items-start gap-2.5 rounded-xl px-2.5 py-2 text-left transition-colors {active
            ? 'bg-brand-soft'
            : 'hover:bg-line-soft'}"
        >
          <span
            class="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-lg {active
              ? 'bg-brand text-brand-fg'
              : 'bg-line-soft text-subtle'}"
          >
            <Icon size={13} strokeWidth={2.25} />
          </span>
          <span class="min-w-0 flex-1">
            <span
              class="block text-[13px] font-semibold {active ? 'text-brand-soft-fg' : 'text-foreground'}"
            >
              {sort}
            </span>
            <span class="block text-[11px] text-subtle">{HUB_SORT_BLURBS[sort]}</span>
          </span>
          {#if active}
            <Check size={14} strokeWidth={2.5} class="mt-1 shrink-0 text-brand-soft-fg" />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
