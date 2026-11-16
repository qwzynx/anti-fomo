<script lang="ts">
  import { ChevronDown, ChevronRight, ChevronUp, Plus, Trash2 } from "$lib/icons";
  import type { ResumeEntry, SectionKind } from "$lib/api";
  import { emptyBullet } from "$lib/resume.svelte";
  import Field from "./Field.svelte";

  /**
   * One entry — a job, a degree, a project — and its bullets.
   *
   * The field labels change with the section, because "Organisation" is the
   * wrong word for a project and "Institution" is the wrong word for a job.
   * The layout is the same underneath: the Harvard format leads with whatever
   * the organisation slot holds, which is the employer for a role and the
   * project's own name for a project.
   *
   * Reordering is up/down buttons, which work the same under a finger.
   *
   * It starts folded to one line — name, role, dates, how many bullets — so a
   * résumé of a dozen entries reads as a list to pick from rather than a wall
   * of forms. `fold` is the page's "open all / fold all": a change of its
   * counter sets every editor to its `open`; between changes each one keeps
   * its own state.
   */
  let {
    entry = $bindable(),
    kind,
    index,
    count,
    onChange,
    onRemove,
    onMove,
    startOpen = false,
    fold = { open: false, n: 0 },
  }: {
    entry: ResumeEntry;
    kind: SectionKind;
    index: number;
    count: number;
    onChange: () => void;
    onRemove: () => void;
    onMove: (delta: number) => void;
    /** Open on arrival — an entry just added has nothing to fold yet. */
    startOpen?: boolean;
    fold?: { open: boolean; n: number };
  } = $props();

  // svelte-ignore state_referenced_locally
  let open = $state(startOpen || fold.open);
  // svelte-ignore state_referenced_locally
  let seen = fold.n;
  $effect(() => {
    if (fold.n === seen) return;
    seen = fold.n;
    open = fold.open;
  });

  const summary = $derived(
    [
      entry.org.trim() && entry.title.trim() ? entry.title.trim() : "",
      [entry.start.trim(), entry.end.trim()].filter(Boolean).join(" – "),
    ]
      .filter(Boolean)
      .join(" · ") || (kind === "skills" ? (entry.detail ?? "") : ""),
  );

  // Projects, jobs and activities share a "Skills & tools" line — what the
  // work was done with, printed under the role — which a tailored version can
  // trim item by item. Under a degree or an award the same slot is one detail.
  const SKILLS = "Skills & tools";
  const words = $derived(
    kind === "education"
      ? { org: "School", title: "Degree & field", detail: "Detail (GPA, honours)" }
      : kind === "projects"
        ? { org: "Project", title: "Your role", detail: SKILLS }
        : kind === "awards"
          ? { org: "Awarded by", title: "Award", detail: "Detail" }
          : kind === "experience" || kind === "leadership"
            ? { org: "Organisation", title: "Role", detail: SKILLS }
            : { org: "Organisation", title: "Role", detail: "Detail" },
  );
  const skillsLine = $derived(words.detail === SKILLS);

  const heading = $derived(entry.org.trim() || entry.title.trim() || "Untitled");

  function addBullet() {
    entry.bullets = [...entry.bullets, emptyBullet()];
    onChange();
  }

  function removeBullet(at: number) {
    entry.bullets = entry.bullets.filter((_, i) => i !== at);
    onChange();
  }

  function moveBullet(at: number, delta: number) {
    const to = at + delta;
    if (to < 0 || to >= entry.bullets.length) return;
    const next = [...entry.bullets];
    [next[at], next[to]] = [next[to], next[at]];
    entry.bullets = next;
    onChange();
  }
</script>

<div class="rounded-xl border border-line-soft bg-line-soft/40 {open ? 'p-4' : 'px-2 py-1.5'}">
  <div class="flex items-center justify-between gap-2 {open ? 'mb-3' : ''}">
    <button
      onclick={() => (open = !open)}
      aria-expanded={open}
      class="flex min-w-0 flex-1 items-center gap-2 rounded-md py-1 text-left {open ? '' : 'pl-1'}"
    >
      <ChevronRight
        size={15}
        class="shrink-0 text-subtle transition-transform {open ? 'rotate-90' : ''}"
      />
      <span class="min-w-0 flex-1">
        <span class="block truncate text-sm font-bold">{heading}</span>
        {#if !open && summary}
          <span class="block truncate text-[11.5px] text-subtle">{summary}</span>
        {/if}
      </span>
      {#if !open && entry.bullets.length > 0}
        <span class="shrink-0 rounded bg-hover px-1.5 py-0.5 text-[11px] font-medium text-muted tabular-nums">
          {entry.bullets.length} {entry.bullets.length === 1 ? "bullet" : "bullets"}
        </span>
      {/if}
    </button>
    <div class="flex shrink-0 items-center gap-0.5">
      <button
        onclick={() => onMove(-1)}
        disabled={index === 0}
        aria-label="Move up"
        class="tap h-8 w-8 disabled:opacity-30"
      >
        <ChevronUp size={16} />
      </button>
      <button
        onclick={() => onMove(1)}
        disabled={index === count - 1}
        aria-label="Move down"
        class="tap h-8 w-8 disabled:opacity-30"
      >
        <ChevronDown size={16} />
      </button>
      <button
        onclick={onRemove}
        aria-label="Remove {heading}"
        class="tap h-8 w-8 hover:text-danger"
      >
        <Trash2 size={16} />
      </button>
    </div>
  </div>

  {#if open}
  <div class="grid gap-3 sm:grid-cols-2">
    <Field label={words.org} bind:value={entry.org} oninput={onChange} placeholder="Acme Corp" />
    <Field
      label={words.title}
      bind:value={entry.title}
      oninput={onChange}
      placeholder="Software Engineer Intern"
    />
    <Field
      label="Location"
      bind:value={entry.location}
      oninput={onChange}
      placeholder="Toronto, ON"
    />
    <div class="grid grid-cols-2 gap-2">
      <Field label="From" bind:value={entry.start} oninput={onChange} placeholder="May 2025" />
      <Field label="To" bind:value={entry.end} oninput={onChange} placeholder="Aug 2025" />
    </div>
    <Field
      label="Link"
      type="url"
      autocomplete="off"
      bind:value={() => entry.link ?? "", (v) => (entry.link = v.trim() ? v.trim() : null)}
      oninput={onChange}
      placeholder={kind === "projects" ? "github.com/you/project" : "company.com"}
      hint="Optional. Makes the name a clickable link in the PDF."
    />
    <div class="sm:col-span-2">
      <!-- `detail` is nullable in the model — an absent detail line and an
           empty one mean the same thing on paper, and storing "" would put a
           blank line into every exported document. A function binding keeps
           the field a plain string while the model keeps the null. -->
      <Field
        label={words.detail}
        bind:value={() => entry.detail ?? "", (v) => (entry.detail = v.trim() ? v : null)}
        oninput={onChange}
        placeholder={skillsLine
          ? kind === "projects"
            ? "Rust · Tauri · SQLite"
            : "Python · SQL · Figma"
          : kind === "education"
            ? "GPA 3.9/4.0"
            : ""}
        hint={skillsLine
          ? "Optional. One line under the role — separate items with · or commas. A tailored version can leave any of them out."
          : undefined}
      />
    </div>
  </div>

  <div class="mt-4">
    <p class="mb-2 text-xs font-semibold tracking-wide text-subtle uppercase">
      Bullets
      {#if entry.bullets.length > 0}
        <span class="ml-1 font-medium normal-case">({entry.bullets.length})</span>
      {/if}
    </p>

    {#each entry.bullets as bullet, i (i)}
      <div class="mb-2 rounded-lg bg-surface p-2.5">
        <div class="flex items-start gap-2">
          <textarea
            bind:value={bullet.text}
            oninput={onChange}
            rows="2"
            placeholder="Start with a verb — “Cut p95 latency 40% by…”"
            aria-label="Bullet {i + 1}"
            class="control control-focus min-w-0 flex-1 resize-y text-[13px] leading-relaxed"
          ></textarea>
          <div class="flex shrink-0 flex-col gap-0.5">
            <button
              onclick={() => moveBullet(i, -1)}
              disabled={i === 0}
              aria-label="Move bullet up"
              class="tap h-7 w-7 disabled:opacity-30"
            >
              <ChevronUp size={14} />
            </button>
            <button
              onclick={() => moveBullet(i, 1)}
              disabled={i === entry.bullets.length - 1}
              aria-label="Move bullet down"
              class="tap h-7 w-7 disabled:opacity-30"
            >
              <ChevronDown size={14} />
            </button>
            <button
              onclick={() => removeBullet(i)}
              aria-label="Remove bullet {i + 1}"
              class="tap h-7 w-7 hover:text-danger"
            >
              <Trash2 size={14} />
            </button>
          </div>
        </div>

        <!-- The skills Rust read out of this bullet's own words. Shown because
             they are what a posting is matched against: seeing "Rust, SQL"
             appear as you type is the feedback that explains why one bullet
             gets picked for a job and another does not. They arrive on save,
             so a freshly typed bullet shows none until the write lands. -->
        {#if bullet.skills.length > 0}
          <div class="mt-1.5 flex flex-wrap gap-1">
            {#each bullet.skills as skill (skill)}
              <span
                class="rounded-md bg-brand-soft px-1.5 py-0.5 text-[10px] font-semibold text-brand-soft-fg"
              >
                {skill}
              </span>
            {/each}
          </div>
        {/if}
      </div>
    {/each}

    <button
      onclick={addBullet}
      class="inline-flex items-center gap-1 text-xs font-semibold text-brand hover:underline"
    >
      <Plus size={13} strokeWidth={2.5} />
      Add a bullet
    </button>
  </div>
{/if}
</div>
