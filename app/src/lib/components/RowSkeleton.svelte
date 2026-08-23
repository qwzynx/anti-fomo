<script lang="ts">
  // Mirrors ItemRow's real geometry rather than showing a grey block, so the
  // page does not visibly reflow when the rows arrive.
  let { count = 6 }: { count?: number } = $props();
</script>

<div>
  {#each { length: count } as _, i (i)}
    <div class="flex animate-pulse border-t border-line-soft first:border-t-transparent items-center gap-3.5 px-1.5 py-2.5" style="animation-delay: {i * 60}ms">
      <div class="h-[34px] w-[34px] shrink-0 rounded-[10px] bg-line"></div>
      <div class="min-w-0 flex-1">
        <div class="mb-1.5 h-3.5 w-2/5 rounded bg-line"></div>
        <div class="h-3 w-1/4 rounded bg-line-soft"></div>
      </div>
      <div class="hidden h-3 w-[150px] shrink-0 rounded bg-line-soft sm:block lg:w-[220px]"></div>
      <div class="hidden h-3 w-[50px] shrink-0 rounded bg-line-soft sm:block"></div>
    </div>
  {/each}
</div>
