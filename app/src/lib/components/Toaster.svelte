<script lang="ts">
  import { CircleAlert, CircleCheck, X } from "$lib/icons";
  import { toast, type Toast } from "$lib/toast.svelte";

  // Bottom-centre, above the phone tab bar, and outside the `inert` shell so an
  // Undo is still reachable while the setup form is up. Errors are announced
  // assertively; everything else politely.

  async function run(t: Toast) {
    toast.dismiss(t.id);
    await t.action?.run();
  }
</script>

<div
  class="pointer-events-none fixed inset-x-0 bottom-20 z-[110] flex flex-col items-center gap-2 px-4 md:bottom-6"
  aria-live="polite"
  role="status"
>
  {#each toast.all as t (t.id)}
    <div
      class="animate-fade-up pointer-events-auto flex w-full max-w-md items-center gap-3 rounded-lg border px-3.5 py-2.5 text-[13px] shadow-lg shadow-black/10
             {t.kind === 'error'
        ? 'border-danger/30 bg-danger-soft text-danger'
        : 'border-line bg-elevated text-foreground'}"
      role={t.kind === "error" ? "alert" : undefined}
      onmouseenter={() => toast.hold(t.id)}
      onmouseleave={() => toast.release(t.id)}
      onfocusin={() => toast.hold(t.id)}
      onfocusout={() => toast.release(t.id)}
    >
      {#if t.kind === "error"}
        <CircleAlert size={16} class="shrink-0" />
      {:else if t.kind === "success"}
        <CircleCheck size={16} class="shrink-0 text-job" />
      {/if}
      <p class="min-w-0 flex-1 leading-snug">{t.message}</p>
      {#if t.action}
        <button
          onclick={() => run(t)}
          class="shrink-0 rounded-md px-2 py-1 font-semibold text-brand hover:bg-brand-soft"
        >
          {t.action.label}
        </button>
      {/if}
      <button
        onclick={() => toast.dismiss(t.id)}
        aria-label="Dismiss notification"
        class="-mr-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-subtle hover:bg-line-soft hover:text-foreground"
      >
        <X size={14} />
      </button>
    </div>
  {/each}
</div>
