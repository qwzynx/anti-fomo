<script lang="ts">
  import { untrack } from "svelte";
  import type { ApplicationStatus, ScrapedItem } from "$lib/api";
  import { APPLICATION_STAGES, stageLabel } from "$lib/applications";
  import { feed } from "$lib/feed.svelte";
  import { Check } from "$lib/icons";
  import { monthDay } from "$lib/item";

  // Where your application to this role stands, and a note to yourself about
  // it. Lives in the posting itself, because that is where you are when you
  // apply; the Saved page groups by it. Choosing a stage saves the role.
  let { item }: { item: ScrapedItem } = $props();

  const app = $derived(feed.application(item.url));
  const status = $derived(app?.status ?? null);

  let note = $state("");
  let noteTimer: ReturnType<typeof setTimeout> | undefined;
  /** Shown only once there is something to note, or the reader asks for it. */
  let noteOpen = $state(false);

  // A different posting: show what is stored for it. Keyed on the URL alone —
  // re-reading on every change to the tracker would close a note the reader
  // had just opened, and race the text they are typing into it.
  $effect(() => {
    const url = item.url;
    untrack(() => {
      note = feed.application(url)?.note ?? "";
      noteOpen = note !== "";
    });
  });

  function pick(next: ApplicationStatus) {
    void feed.setApplication(item, status === next ? null : next, note || null);
  }

  function onNote(value: string) {
    note = value;
    clearTimeout(noteTimer);
    noteTimer = setTimeout(() => void feed.setApplication(item, status, value || null), 600);
  }

  const since = $derived.by(() => {
    if (!app?.updated_at || !status) return null;
    const d = monthDay(app.updated_at);
    return `${d.month} ${Number(d.day)}`;
  });
</script>

<section class="mb-4 rounded-xl border border-line-soft p-4" aria-labelledby="tracker-{item.url}">
  <div class="mb-2.5 flex items-baseline justify-between gap-2">
    <p
      id="tracker-{item.url}"
      class="text-[11px] font-bold tracking-wide text-subtle uppercase"
    >
      Your application
    </p>
    {#if status && since}
      <p class="text-[11px] text-subtle">{stageLabel(status)} since {since}</p>
    {/if}
  </div>
  <div class="flex flex-wrap gap-1.5" role="group" aria-label="Application stage">
    {#each APPLICATION_STAGES as stage (stage.value)}
      {@const active = status === stage.value}
      <button
        onclick={() => pick(stage.value)}
        aria-pressed={active}
        class="inline-flex h-8 items-center gap-1 rounded-full px-3 text-[13px] font-semibold transition-colors
               {active ? stage.activeClass : 'border border-line text-muted hover:border-subtle'}"
      >
        {#if active}<Check size={12} strokeWidth={3} />{/if}
        {stage.label}
      </button>
    {/each}
    {#if !noteOpen}
      <button
        onclick={() => (noteOpen = true)}
        class="h-8 rounded-full px-2 text-[13px] font-semibold text-brand hover:underline"
      >
        Add note
      </button>
    {/if}
  </div>
  {#if noteOpen}
    <textarea
      value={note}
      oninput={(e) => onNote(e.currentTarget.value)}
      rows="2"
      maxlength="2000"
      placeholder="Referral, recruiter, interview dates, follow-ups…"
      aria-label="Note on this application"
      class="control control-focus mt-3 block w-full resize-y text-sm leading-relaxed"
    ></textarea>
  {/if}
</section>
