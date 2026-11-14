<script lang="ts">
  import { confirmState } from "$lib/confirm.svelte";
  import { CircleAlert } from "$lib/icons";
  import Modal from "./Modal.svelte";

  const req = $derived(confirmState.current);
  let confirmButton = $state<HTMLButtonElement | null>(null);

  // Focus lands on Cancel for anything destructive, on the action otherwise:
  // a stray Enter should never be the thing that deletes.
  let cancelButton = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    if (!req) return;
    setTimeout(() => (req.danger ? cancelButton : confirmButton)?.focus(), 0);
  });
</script>

<Modal open={req !== null} onClose={() => confirmState.answer(false)} titleId="confirm-title">
  {#if req}
    <div class="p-6">
      <div class="flex items-start gap-3">
        {#if req.danger}
          <span
            class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-danger-soft text-danger"
          >
            <CircleAlert size={20} />
          </span>
        {/if}
        <div class="min-w-0">
          <h2 id="confirm-title" class="text-lg font-bold">{req.title}</h2>
          {#if req.body}
            <p class="mt-1.5 text-sm text-muted">{req.body}</p>
          {/if}
        </div>
      </div>
      <div class="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <button
          bind:this={cancelButton}
          onclick={() => confirmState.answer(false)}
          class="h-11 rounded-xl border border-line px-4 font-semibold transition-colors hover:bg-line-soft sm:h-10"
        >
          {req.cancelLabel ?? "Cancel"}
        </button>
        <button
          bind:this={confirmButton}
          onclick={() => confirmState.answer(true)}
          class="h-11 rounded-xl px-4 font-semibold transition-colors sm:h-10
                 {req.danger
            ? 'bg-danger text-danger-fg hover:opacity-90'
            : 'bg-brand text-brand-fg hover:bg-brand-hover'}"
        >
          {req.confirmLabel}
        </button>
      </div>
    </div>
  {/if}
</Modal>
