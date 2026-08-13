<script lang="ts">
  import type { IconComponent } from "$lib/icons";

  // One empty state for every list in the app: same shape whether the cause is
  // an over-tight filter, an empty cache, or a genuinely empty saved list.
  let {
    icon,
    title,
    body,
    actionLabel,
    onAction,
    busy = false,
  }: {
    icon: IconComponent;
    title: string;
    body?: string;
    actionLabel?: string;
    onAction?: () => void;
    busy?: boolean;
  } = $props();

  const Icon = $derived(icon);
</script>

<div class="flex flex-col items-center rounded-3xl border border-dashed border-line bg-surface px-6 py-20 text-center">
  <div class="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl bg-line-soft text-subtle">
    <Icon size={22} />
  </div>
  <p class="font-display text-lg font-bold">{title}</p>
  {#if body}
    <p class="mt-1 max-w-sm text-sm text-muted">{body}</p>
  {/if}
  {#if actionLabel && onAction}
    <button
      onclick={onAction}
      disabled={busy}
      class="mt-5 rounded-xl bg-brand px-5 py-2.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
    >
      {busy ? "Working…" : actionLabel}
    </button>
  {/if}
</div>
