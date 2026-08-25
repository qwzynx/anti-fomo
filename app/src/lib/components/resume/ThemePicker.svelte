<script lang="ts">
  import { Check } from "$lib/icons";
  import { cssColor, type ResumeTheme, type ResumeThemeOption, type Rgb } from "$lib/api";

  /**
   * Which of the four versions, in which colour, at what size.
   *
   * The versions all lay out identically — the Harvard format — and differ in
   * treatment: the family, the weight of the rules, and *where* the accent is
   * allowed to land. That split is why a free colour choice is safe: a theme
   * decides which elements take colour (`accent_roles` in Rust), so no colour
   * can end up behind body text.
   *
   * The swatches are the app's own accent tokens, resolved from CSS rather than
   * hard-coded, so a palette change in `app.css` reaches this row too. The hex
   * field is the escape hatch for anything else.
   */
  let {
    theme,
    options,
    onChange,
  }: {
    theme: ResumeTheme;
    options: ResumeThemeOption[];
    onChange: (next: ResumeTheme) => void;
  } = $props();

  /** Named after the tokens in `app.css`, read at runtime so they stay in step. */
  const SWATCH_TOKENS = ["--brand", "--job", "--event", "--star", "--article"] as const;

  let swatches = $state<Rgb[]>([]);

  $effect(() => {
    const styles = getComputedStyle(document.documentElement);
    const parsed = SWATCH_TOKENS.map((token) => parseColor(styles.getPropertyValue(token))).filter(
      (c): c is Rgb => c !== null,
    );
    // Black last: a résumé in plain black ink is a legitimate choice, not an
    // absence of one, and the Classic theme ignores the accent entirely.
    swatches = [...parsed, { r: 24, g: 24, b: 27 }];
  });

  /** `#rrggbb`, `#rgb` and `rgb(r g b)` — whatever the token happens to hold. */
  function parseColor(raw: string): Rgb | null {
    const value = raw.trim();
    if (!value) return null;
    const hex = value.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
    if (hex) {
      const h = hex[1];
      const full = h.length === 3 ? [...h].map((c) => c + c).join("") : h;
      return {
        r: parseInt(full.slice(0, 2), 16),
        g: parseInt(full.slice(2, 4), 16),
        b: parseInt(full.slice(4, 6), 16),
      };
    }
    const rgb = value.match(/(\d+)[,\s]+(\d+)[,\s]+(\d+)/);
    if (rgb) return { r: +rgb[1], g: +rgb[2], b: +rgb[3] };
    return null;
  }

  function toHex(c: Rgb): string {
    return `#${[c.r, c.g, c.b].map((n) => n.toString(16).padStart(2, "0")).join("")}`;
  }

  const same = (a: Rgb, b: Rgb) => a.r === b.r && a.g === b.g && a.b === b.b;

  /** Picking a version keeps the colour and the sizing the user already set. */
  function pickVersion(option: ResumeThemeOption) {
    onChange({
      ...option.theme,
      accent: theme.accent,
      page: theme.page,
      base_size: option.theme.base_size,
      margin: theme.margin,
      max_pages: theme.max_pages,
    });
  }

  const step = (field: "base_size" | "margin", delta: number) => {
    onChange({ ...theme, [field]: +(theme[field] + delta).toFixed(2) });
  };

  /** Whether this version does anything with colour at all. */
  const usesAccent = (t: ResumeTheme) => Object.values(t.accent_roles).some(Boolean);
</script>

<div class="space-y-5">
  <div>
    <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Version</p>
    <div class="grid grid-cols-2 gap-2">
      {#each options as option (option.id)}
        {@const active = theme.id === option.id}
        <button
          onclick={() => pickVersion(option)}
          aria-pressed={active}
          class="rounded-xl border p-3 text-left transition-colors {active
            ? 'border-brand bg-brand-soft'
            : 'border-line hover:border-brand'}"
        >
          <span class="flex items-center gap-1.5 text-sm font-bold">
            {#if active}<Check size={13} strokeWidth={3} />{/if}
            {option.label}
          </span>
          <!-- A three-line sketch of what the version does to a heading, so
               the names mean something before you click one. -->
          <span class="mt-2 block space-y-1" aria-hidden="true">
            <span
              class="block h-2 rounded-sm px-1 text-[0px]"
              style:background={option.theme.heading === "band"
                ? cssColor(mix(theme.accent, 0.84))
                : "transparent"}
            >
              <span
                class="block h-2 w-1/3 rounded-xs"
                style:background={option.theme.accent_roles.headings
                  ? cssColor(theme.accent)
                  : "rgb(24 24 27)"}
              ></span>
            </span>
            {#if option.theme.heading === "rule"}
              <span
                class="block h-px w-full"
                style:background={option.theme.accent_roles.rules
                  ? cssColor(theme.accent)
                  : "rgb(24 24 27)"}
              ></span>
            {/if}
            <span class="block h-1.5 w-full rounded-xs bg-line"></span>
            <span class="block h-1.5 w-4/5 rounded-xs bg-line"></span>
          </span>
          <span class="mt-2 block text-[11px] text-muted">
            {option.theme.family === "serif" ? "Serif" : "Sans"}
            {usesAccent(option.theme) ? " · colour" : " · black ink"}
          </span>
        </button>
      {/each}
    </div>
  </div>

  <div>
    <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Colour</p>
    {#if usesAccent(theme)}
      <div class="flex flex-wrap items-center gap-2">
        {#each swatches as swatch (toHex(swatch))}
          <button
            onclick={() => onChange({ ...theme, accent: swatch })}
            aria-label="Use {toHex(swatch)}"
            aria-pressed={same(theme.accent, swatch)}
            class="h-8 w-8 rounded-full ring-offset-2 ring-offset-surface transition-shadow {same(
              theme.accent,
              swatch,
            )
              ? 'ring-2 ring-brand'
              : 'ring-1 ring-line'}"
            style:background={cssColor(swatch)}
          ></button>
        {/each}
        <label class="ml-1 inline-flex items-center gap-2 text-xs text-muted">
          <span class="sr-only">Custom colour</span>
          <input
            type="color"
            value={toHex(theme.accent)}
            oninput={(e) => {
              const parsed = parseColor(e.currentTarget.value);
              if (parsed) onChange({ ...theme, accent: parsed });
            }}
            class="h-8 w-10 cursor-pointer rounded-md border border-line bg-surface p-0.5"
          />
          Custom
        </label>
      </div>
    {:else}
      <p class="text-xs text-muted">
        The Classic version is black ink throughout — pick another version to choose a colour.
      </p>
    {/if}
  </div>

  <div class="grid grid-cols-2 gap-4">
    <div>
      <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Text size</p>
      <div class="flex items-center gap-2">
        <button onclick={() => step("base_size", -0.5)} class="tap h-8 w-8 border border-line">
          −
        </button>
        <span class="min-w-12 text-center text-sm font-semibold">{theme.base_size}pt</span>
        <button onclick={() => step("base_size", 0.5)} class="tap h-8 w-8 border border-line">
          +
        </button>
      </div>
    </div>
    <div>
      <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Margins</p>
      <div class="flex items-center gap-2">
        <button onclick={() => step("margin", -6)} class="tap h-8 w-8 border border-line">−</button>
        <span class="min-w-12 text-center text-sm font-semibold">
          {(theme.margin / 72).toFixed(2)}″
        </span>
        <button onclick={() => step("margin", 6)} class="tap h-8 w-8 border border-line">+</button>
      </div>
    </div>
  </div>

  <div class="grid grid-cols-2 gap-4">
    <div>
      <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Paper</p>
      <div class="flex gap-2">
        {#each [{ id: "letter", label: "Letter" }, { id: "a4", label: "A4" }] as const as size (size.id)}
          <button
            onclick={() => onChange({ ...theme, page: size.id })}
            aria-pressed={theme.page === size.id}
            class="chip px-3 py-1.5 text-xs {theme.page === size.id
              ? 'bg-brand text-brand-fg'
              : 'border border-line text-muted hover:border-brand'}"
          >
            {size.label}
          </button>
        {/each}
      </div>
    </div>
    <div>
      <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">Page limit</p>
      <div class="flex gap-2">
        {#each [1, 2] as limit (limit)}
          <button
            onclick={() => onChange({ ...theme, max_pages: limit })}
            aria-pressed={theme.max_pages === limit}
            class="chip px-3 py-1.5 text-xs {theme.max_pages === limit
              ? 'bg-brand text-brand-fg'
              : 'border border-line text-muted hover:border-brand'}"
          >
            {limit} page{limit > 1 ? "s" : ""}
          </button>
        {/each}
      </div>
    </div>
  </div>
</div>

<script lang="ts" module>
  import type { Rgb as RgbType } from "$lib/api";

  /** The accent mixed toward white — mirrors `Rgb::tint` in `theme.rs`, so the
      version sketch above shows the band colour the PDF will actually use. */
  function mix(c: RgbType, amount: number): RgbType {
    const m = (n: number) => Math.round(n + (255 - n) * amount);
    return { r: m(c.r), g: m(c.g), b: m(c.b) };
  }
  export { mix };
</script>
