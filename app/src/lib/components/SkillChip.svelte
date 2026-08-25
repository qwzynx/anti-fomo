<script lang="ts">
  import { Check, Plus } from "$lib/icons";

  // The one have / don't-have control, used by the skills form, the settings
  // preview and the per-job match block. A component rather than the repo's
  // usual local `chipClass()` helper because this chip carries state and an
  // icon, and three hand-copied versions would drift.
  //
  // A button with `aria-pressed`, not a checkbox: there is not one
  // `<input type="checkbox">` in this codebase and this is not the place to
  // introduce the first.
  let {
    skill,
    has,
    onToggle,
    size = "md",
    optional = false,
  }: {
    skill: string;
    has: boolean;
    onToggle: (skill: string) => void;
    /** "sm" for the dense grid inside a job detail. */
    size?: "sm" | "md";
    /**
     * The posting asked for this one under "nice to have". Drawn dashed, and
     * said out loud in the label — a border weight is not something a screen
     * reader can pass on.
     */
    optional?: boolean;
  } = $props();

  const sizing = $derived(size === "sm" ? "px-2.5 py-1 text-xs" : "px-3.5 py-2 text-sm");
  const iconSize = $derived(size === "sm" ? 12 : 14);
  const note = $derived(optional ? " (nice to have)" : "");
  // A filled chip has no border to dash, so the optional version borrows one
  // in the fill's own foreground rather than the transparent placeholder a
  // required chip carries to keep the two the same height.
  const tone = $derived(
    has
      ? optional
        ? "border border-dashed border-brand-fg/50 bg-brand text-brand-fg"
        : "border border-transparent bg-brand text-brand-fg"
      : optional
        ? "border border-dashed border-line bg-surface text-muted hover:border-brand hover:text-foreground"
        : "border border-line bg-surface text-muted hover:border-brand hover:text-foreground",
  );
</script>

<button
  onclick={() => onToggle(skill)}
  aria-pressed={has}
  aria-label={has
    ? `${skill}${note} — you have this, tap to remove`
    : `${skill}${note} — tap to add`}
  class="chip inline-flex items-center gap-1.5 {sizing} {tone}"
>
  {#if has}
    <Check size={iconSize} strokeWidth={3} />
  {:else}
    <Plus size={iconSize} strokeWidth={2.5} class="text-subtle" />
  {/if}
  {skill}
</button>
