<script lang="ts">
  import { timeAgo } from "$lib/api";
  import { feed } from "$lib/feed.svelte";
  import { DISCIPLINES } from "$lib/filters";
  import { theme, type Theme } from "$lib/theme.svelte";

  const MAJORS = DISCIPLINES.filter((d) => d !== "All");
  const THEMES: { value: Theme; label: string }[] = [
    { value: "light", label: "☀️ Light" },
    { value: "dark", label: "🌙 Dark" },
    { value: "system", label: "🖥️ System" },
  ];

  const sourceCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const item of feed.items) {
      counts.set(item.source_platform, (counts.get(item.source_platform) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  });
</script>

<main class="mx-auto w-full max-w-3xl flex-1 px-4 py-8 sm:px-6 sm:py-10">
  <h2 class="mb-1 text-3xl font-bold">Settings</h2>
  <p class="mb-8 text-sm text-zinc-500 dark:text-zinc-400">
    Everything is stored locally on this device. No account, no server.
  </p>

  <section
    class="mb-5 rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900"
  >
    <h3 class="mb-1 text-lg font-bold">Your field</h3>
    <p class="mb-4 text-sm text-zinc-500 dark:text-zinc-400">
      Items matching your field are ranked to the top of the feed.
    </p>
    <div class="flex flex-wrap gap-2">
      {#each MAJORS as option (option)}
        <button
          onclick={() => feed.setMajor(option)}
          class="rounded-full px-4 py-2 text-sm font-semibold transition-colors {feed.major ===
          option
            ? 'bg-indigo-600 text-white'
            : 'border border-zinc-200 text-zinc-600 hover:border-indigo-400 dark:border-zinc-800 dark:text-zinc-400'}"
        >
          {option}
        </button>
      {/each}
    </div>
  </section>

  <section
    class="mb-5 rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900"
  >
    <h3 class="mb-1 text-lg font-bold">Appearance</h3>
    <p class="mb-4 text-sm text-zinc-500 dark:text-zinc-400">
      "System" follows your OS setting and updates live.
    </p>
    <div class="flex flex-wrap gap-2">
      {#each THEMES as option (option.value)}
        <button
          onclick={() => theme.set(option.value)}
          class="rounded-full px-4 py-2 text-sm font-semibold transition-colors {theme.value ===
          option.value
            ? 'bg-indigo-600 text-white'
            : 'border border-zinc-200 text-zinc-600 hover:border-indigo-400 dark:border-zinc-800 dark:text-zinc-400'}"
        >
          {option.label}
        </button>
      {/each}
    </div>
  </section>

  <section
    class="mb-5 rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900"
  >
    <div class="mb-4 flex items-start justify-between gap-4">
      <div>
        <h3 class="mb-1 text-lg font-bold">Sources</h3>
        <p class="text-sm text-zinc-500 dark:text-zinc-400">
          {feed.status?.item_count ?? 0} items cached · last refreshed
          {feed.status?.last_refresh ? timeAgo(feed.status.last_refresh) : "never"}
        </p>
      </div>
      <button
        onclick={() => feed.refresh(true)}
        disabled={feed.refreshing}
        class="shrink-0 rounded-xl bg-indigo-600 px-4 py-2 text-sm font-semibold text-white transition-colors hover:bg-indigo-500 disabled:opacity-50"
      >
        {feed.refreshing ? "Refreshing…" : "Refresh now"}
      </button>
    </div>

    {#if sourceCounts.length > 0}
      <ul class="flex flex-col divide-y divide-zinc-100 dark:divide-zinc-800">
        {#each sourceCounts as [source, count] (source)}
          <li class="flex items-center justify-between py-2 text-sm">
            <span class="font-medium">{source}</span>
            <span class="text-zinc-400">{count} in feed</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="text-sm text-zinc-500">Nothing cached yet.</p>
    {/if}
  </section>

  <section
    class="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900"
  >
    <h3 class="mb-1 text-lg font-bold">About</h3>
    <p class="text-sm leading-relaxed text-zinc-500 dark:text-zinc-400">
      Anti-FOMO scrapes ten public sources directly on your device and ranks them against your
      field. Results are cached locally, so the feed opens instantly and stays readable offline.
    </p>
  </section>
</main>
