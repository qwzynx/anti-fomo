<script lang="ts">
  import { logoFor, timeAgo, type ScrapedItem } from "$lib/api";
  import { splitTitle, tagsFor, typeBadgeClass } from "$lib/item";

  let {
    item,
    onOpen,
    index = 0,
  }: { item: ScrapedItem; onOpen: (item: ScrapedItem) => void; index?: number } = $props();

  let logoFailed = $state(false);

  const logo = $derived(logoFor(item.url));
  const parts = $derived(splitTitle(item));
  const tags = $derived(tagsFor(item));
</script>

<button
  onclick={() => onOpen(item)}
  style="animation-delay: {Math.min(index, 12) * 30}ms"
  class="animate-fade-up group relative flex w-full flex-col gap-3 rounded-2xl border border-zinc-200 bg-white p-5 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-indigo-500 hover:shadow-lg hover:shadow-indigo-600/5 dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-400"
>
  <div class="flex items-start justify-between gap-2">
    <div class="flex items-center gap-2.5">
      {#if logo && !logoFailed}
        <img
          src={logo}
          alt=""
          width="36"
          height="36"
          onerror={() => (logoFailed = true)}
          class="h-9 w-9 rounded-lg bg-zinc-100 object-contain p-1 dark:bg-zinc-800"
        />
      {:else}
        <div
          class="flex h-9 w-9 items-center justify-center rounded-lg bg-zinc-100 text-sm font-bold text-zinc-500 dark:bg-zinc-800"
        >
          {parts.primary.charAt(0).toUpperCase()}
        </div>
      {/if}
      <span
        class="rounded-full px-2 py-0.5 text-[10px] font-bold tracking-wider uppercase {typeBadgeClass(
          item.item_type,
        )}"
      >
        {item.item_type}
      </span>
    </div>
    {#if (item.relevance_score ?? 0) >= 10}
      <span
        class="shrink-0 rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-bold text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
      >
        ⭐ TOP MATCH
      </span>
    {/if}
  </div>

  <div class="flex flex-col gap-0.5">
    <h3
      class="line-clamp-2 text-lg leading-tight font-bold group-hover:text-indigo-600 dark:group-hover:text-indigo-400"
    >
      {parts.primary}
    </h3>
    {#if parts.secondary}
      <p class="line-clamp-1 text-sm font-medium text-zinc-600 dark:text-zinc-400">
        {parts.secondary}
      </p>
    {/if}
    <span class="mt-0.5 text-xs font-medium text-zinc-400">{item.source_platform}</span>
  </div>

  {#if !parts.secondary}
    <p class="line-clamp-2 text-sm leading-relaxed text-zinc-600 dark:text-zinc-400">
      {item.content_text || "Open for details."}
    </p>
  {/if}

  {#if tags.length > 0}
    <div class="flex flex-wrap gap-1.5">
      {#each tags as tag (tag)}
        <span
          class="rounded-md bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400"
        >
          {tag}
        </span>
      {/each}
    </div>
  {/if}

  <div
    class="mt-auto flex items-center justify-between border-t border-zinc-100 pt-3 text-[11px] font-medium text-zinc-400 dark:border-zinc-800"
  >
    <span>{timeAgo(item.timestamp)}</span>
    <span class="text-indigo-500 opacity-0 transition-opacity group-hover:opacity-100">
      View details →
    </span>
  </div>
</button>
