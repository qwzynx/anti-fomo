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

<div class="flex flex-col items-center px-6 py-16 text-center">
  <div class="mb-3 flex h-10 w-10 items-center justify-center rounded-lg border border-line bg-surface text-subtle">
    <Icon size={18} />
  </div>
  <p class="text-[14px] font-semibold">{title}</p>
  {#if body}
    <p class="mt-1 max-w-sm text-[13px] text-muted">{body}</p>
  {/if}
  {#if actionLabel && onAction}
    <button onclick={onAction} disabled={busy} class="btn-primary mt-4">
      {busy ? "Working…" : actionLabel}
    </button>
  {/if}
</div>
