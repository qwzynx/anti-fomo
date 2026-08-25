<script lang="ts">
  import { cssColor, type ResumePage, type ResumeFontStyle } from "$lib/api";

  /**
   * The résumé, drawn from the same positioned boxes the PDF writer draws.
   *
   * **SVG, not positioned divs.** An `<svg><text y>` puts the baseline exactly
   * at `y`, which is what PDF's `Tm` operator does too — so a box drawn here
   * and the same box drawn into the file land in the identical place. HTML
   * would mean reproducing CSS line-box maths (content area, half-leading,
   * the font's own ascent) just to work out where a baseline ends up, and
   * being a fraction of a point out on every line of a document whose whole
   * job is fitting one page.
   *
   * Nothing is measured here either. Rust already wrapped the text and
   * resolved every alignment to a left `x`, so the browser is only asked to
   * paint runs, never to lay them out. That is why there is no `text-anchor`
   * and why every string is a single unwrapped line.
   *
   * The page is deliberately **not** theme-aware. Paper is white and résumé
   * ink is dark whatever the app's theme is, so the surrounding chrome dims in
   * dark mode and the sheet itself does not.
   */
  let {
    pages,
    /** Points per CSS pixel. 1 renders a Letter page 612px wide. */
    scale = 1,
    label = "Résumé preview",
  }: {
    pages: ResumePage[];
    scale?: number;
    label?: string;
  } = $props();

  const FAMILY = {
    serif: '"Resume Serif", "Liberation Serif", "Times New Roman", serif',
    sans: '"Resume Sans", Inter, system-ui, sans-serif',
  } as const;

  function weight(style: ResumeFontStyle): number {
    return style === "bold" || style === "bolditalic" ? 700 : 400;
  }

  function slant(style: ResumeFontStyle): "italic" | "normal" {
    return style === "italic" || style === "bolditalic" ? "italic" : "normal";
  }
</script>

<div class="flex flex-col items-center gap-4" role="img" aria-label={label}>
  {#each pages as page, n (n)}
    <svg
      viewBox="0 0 {page.width} {page.height}"
      width={page.width * scale}
      height={page.height * scale}
      class="max-w-full rounded-sm bg-white shadow-lg ring-1 ring-black/10"
      xmlns="http://www.w3.org/2000/svg"
    >
      <!-- The sheet. Explicit rather than inherited: the app's dark mode must
           not tint the paper. -->
      <rect x="0" y="0" width={page.width} height={page.height} fill="#ffffff" />

      {#each page.items as item, i (i)}
        {#if item.kind === "rect"}
          <rect
            x={item.x}
            y={item.y}
            width={item.w}
            height={item.h}
            fill={cssColor(item.color)}
          />
        {:else if item.kind === "text"}
          <text
            x={item.x}
            y={item.y}
            font-family={FAMILY[item.family]}
            font-size={item.size}
            font-weight={weight(item.style)}
            font-style={slant(item.style)}
            letter-spacing={item.tracking || null}
            fill={cssColor(item.color)}
            xml:space="preserve">{item.text}</text
          >
        {/if}
      {/each}
    </svg>
    {#if pages.length > 1}
      <p class="text-xs font-medium text-subtle">Page {n + 1} of {pages.length}</p>
    {/if}
  {/each}
</div>
