<script lang="ts">
  // Mirrors ItemCard's actual layout rather than showing a plain grey block, so
  // the page doesn't visibly reflow when the real cards arrive.
  let { count = 6 }: { count?: number } = $props();
</script>

<div class="grid grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3">
  {#each { length: count } as _, i (i)}
    <div class="card animate-pulse p-5" style="animation-delay: {i * 60}ms">
      <div class="mb-4 flex items-center gap-2.5">
        <div class="h-9 w-9 rounded-lg bg-line"></div>
        <div class="h-4 w-20 rounded-full bg-line"></div>
      </div>
      <div class="mb-2 h-5 w-4/5 rounded bg-line"></div>
      <div class="mb-4 h-4 w-1/2 rounded bg-line-soft"></div>
      <div class="mb-2 h-3 w-full rounded bg-line-soft"></div>
      <div class="h-3 w-2/3 rounded bg-line-soft"></div>
      <div class="mt-5 flex gap-1.5">
        <div class="h-4 w-14 rounded-md bg-line-soft"></div>
        <div class="h-4 w-20 rounded-md bg-line-soft"></div>
      </div>
    </div>
  {/each}
</div>
