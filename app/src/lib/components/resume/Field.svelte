<script lang="ts">
  /**
   * A labelled text input or textarea.
   *
   * This is the app's first real form control. Until the résumé builder there
   * were exactly two `<input>` elements in the whole codebase — both search
   * boxes — because every other "setting" is a choice from a Rust-owned
   * vocabulary and renders as a `<button aria-pressed>`. Free text is
   * genuinely new here, so it gets one component rather than six ad-hoc
   * styles across two routes.
   *
   * Built on the existing `control` / `control-focus` utilities in `app.css`,
   * so it inherits the app's radius, border and focus ring rather than
   * inventing its own.
   *
   * There is no validation and no submit. Nothing in this app has a save
   * button; edits go straight to the store, which debounces the write.
   */
  let {
    label,
    value = $bindable(""),
    placeholder = "",
    multiline = false,
    rows = 3,
    /** Sits under the field in small type. For a format hint, not an error. */
    hint = "",
    /** Hidden from sight but read out — for a field whose column heading is its label. */
    hideLabel = false,
    oninput,
  }: {
    label: string;
    value?: string;
    placeholder?: string;
    multiline?: boolean;
    rows?: number;
    hint?: string;
    hideLabel?: boolean;
    oninput?: () => void;
  } = $props();

  const id = `field-${Math.random().toString(36).slice(2, 9)}`;
</script>

<div class="min-w-0">
  <label
    for={id}
    class={hideLabel
      ? "sr-only"
      : "mb-1 block text-xs font-semibold tracking-wide text-subtle uppercase"}
  >
    {label}
  </label>
  {#if multiline}
    <textarea
      {id}
      bind:value
      {rows}
      {placeholder}
      {oninput}
      class="control control-focus w-full resize-y leading-relaxed"
    ></textarea>
  {:else}
    <input
      {id}
      bind:value
      type="text"
      {placeholder}
      {oninput}
      class="control control-focus w-full"
    />
  {/if}
  {#if hint}
    <p class="mt-1 text-xs text-subtle">{hint}</p>
  {/if}
</div>

<style>
  /* The one thing `control` does not cover: a textarea defaults to an inline
     baseline, which leaves a stray gap under it inside a flex column. */
  textarea {
    display: block;
  }
</style>
