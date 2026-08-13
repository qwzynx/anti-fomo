<script lang="ts">
  import { page } from "$app/state";
  import { feed } from "$lib/feed.svelte";
  import { NAV } from "$lib/nav";

  // This ships on Android, where a top nav bar is the wrong shape and out of
  // thumb reach. Phones get a tab bar; sm: and up keeps the header nav.
  const savedCount = $derived(feed.status?.saved_count ?? 0);
</script>

<nav
  class="glass safe-bottom fixed inset-x-0 bottom-0 z-40 border-t border-line sm:hidden"
  aria-label="Main"
>
  <div class="flex items-stretch justify-around">
    {#each NAV as link (link.href)}
      {@const active = page.url.pathname === link.href}
      <a
        href={link.href}
        aria-current={active ? "page" : undefined}
        class="relative flex flex-1 flex-col items-center gap-1 py-2.5 text-[10px] font-semibold transition-colors
               {active ? 'text-brand' : 'text-subtle'}"
      >
        <span class="relative">
          <link.icon size={20} strokeWidth={active ? 2.5 : 2} />
          {#if link.href === "/saved" && savedCount > 0}
            <span
              class="absolute -top-1 -right-2 min-w-4 rounded-full bg-brand px-1 text-[9px] leading-4 font-bold text-brand-fg"
            >
              {savedCount > 99 ? "99+" : savedCount}
            </span>
          {/if}
        </span>
        {link.label}
      </a>
    {/each}
  </div>
</nav>
