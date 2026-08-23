<script lang="ts">
  import type { Snippet } from "svelte";

  // Dialog chrome and nothing else: the backdrop, the scroll lock, focus
  // management and the Escape key. Lifted out of ItemModal once the skills
  // form needed the same behaviour — a second hand-rolled focus trap is a
  // second one to get subtly wrong, and this is the part that is easy to get
  // subtly wrong.
  //
  // Mobile-first by design: a sheet rising from the bottom edge below `sm`, a
  // centred dialog above it.
  let {
    open,
    onClose,
    titleId,
    size = "md",
    children,
  }: {
    open: boolean;
    onClose: () => void;
    /** Id of the heading inside `children`, for `aria-labelledby`. */
    titleId?: string;
    /** "md" fits an item; "lg" gives a wall of chips room to breathe. */
    size?: "md" | "lg";
    children: Snippet;
  } = $props();

  let panel = $state<HTMLElement | null>(null);

  const widthClass = $derived(size === "lg" ? "max-w-3xl" : "max-w-xl");

  // The backdrop scroll-locks the page underneath while open.
  $effect(() => {
    if (!open) return;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = "";
    };
  });

  // Move focus into the dialog on open and hand it back on close, so keyboard
  // and screen-reader users aren't dropped at the top of the document.
  $effect(() => {
    if (!open || !panel) return;
    const previous = document.activeElement as HTMLElement | null;
    panel.focus();
    return () => previous?.focus?.();
  });

  /** Everything focusable inside the panel, in document order. */
  function focusables(): HTMLElement[] {
    if (!panel) return [];
    return [
      ...panel.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => el.offsetParent !== null);
  }

  /** Keeps Tab cycling inside the dialog rather than escaping to the page. */
  function trapFocus(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const nodes = focusables();
    if (nodes.length === 0) return;

    const first = nodes[0];
    const last = nodes[nodes.length - 1];
    const active = document.activeElement;

    if (e.shiftKey && (active === first || active === panel)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (!open) return;
    if (e.key === "Escape") onClose();
    else trapFocus(e);
  }}
/>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="animate-fade-in fixed inset-0 z-50 flex items-end justify-center bg-black/50 p-0 backdrop-blur-sm sm:items-center sm:p-6"
    onclick={onClose}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      bind:this={panel}
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
      class="animate-fade-up safe-bottom card flex max-h-[88vh] w-full {widthClass} flex-col overflow-hidden rounded-t-3xl bg-elevated shadow-2xl focus:outline-none sm:rounded-3xl"
    >
      {@render children()}
    </div>
  </div>
{/if}
