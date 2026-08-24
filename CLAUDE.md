# Anti-FOMO Project Guidelines for Claude

## Overview
**Anti-FOMO** consolidates job search (internships, jobs), industry news, and event
tracking into a single ranked feed, so the user stays current without checking a
dozen platforms.

It is a **Tauri 2 desktop and mobile app**: Svelte 5 + Tailwind 4 in the webview,
Rust in the core. Scraping, ranking and storage all happen on-device. There is no
backend service, no API base URL, no CORS, and no user accounts — a single local
SQLite database holds both the cached feed and the user's preferences.

## Architecture

Everything ships from `/app`:

- **`app/src-tauri`** — the Rust core.
  - `scrapers/` — one module per source, all implementing the `Scraper` trait and
    run concurrently. A source that fails logs and returns nothing; it must never
    take the whole refresh down.
    - `details.rs` is the exception: a second phase that runs *after* the scrape
      and works on URLs rather than sources. `route()` returns an ordered chain
      per posting — the employer's ATS API, then the page's schema.org
      `JobPosting`, then simplify.jobs as a mirror — and `fetch()` walks it
      until something returns text. Adding an employer means adding a variant
      there, not a new `Scraper`. `sections.rs` recovers the Requirements /
      Responsibilities / Perks split from an HTML description and is the only
      module that knows those heading words; `jsonld.rs` reads the schema.org
      block. **Every handler feeds its description through `sections::split`** —
      one that strips it to plain text instead throws away both the structure
      the UI renders and the headings that tell requirements from boilerplate.
      Verify with `cargo run --features dev-tools --bin detail_check`.
  - `location.rs`, `rank.rs`, `skills.rs` — pure functions (location tagging,
    discipline classification, interest-tag matching, recency decay, relevance
    scoring, round-robin per-source diversification, skill extraction).
    Unit-tested. **A posting's skills come from its own fetched text and
    nothing else** — its requirements, its duties, its description and the
    tags the source attached, read together. Nothing is inferred from the job
    title, so a posting whose page could not be read reports no skills at all;
    `skills::from_posting` says which case it is, and `hasScrapedPosting` in
    `lib/item.ts` must keep answering the same question, since that is what
    decides whether the UI shows a panel or explains its absence.
  - `db.rs` — SQLite via `rusqlite` (bundled). Items are a rebuildable cache keyed
    by URL. `settings` and `item_state` are the durable tables and must survive a
    schema bump, so they are created with `IF NOT EXISTS` and never dropped.
  - `commands.rs` — the `invoke()` surface the UI calls. **Every command that
    touches the database is `#[tauri::command(async)]`.** A plain
    `#[tauri::command]` on a synchronous function runs on the *main thread*,
    which is the webview's thread, so a read that walks the cache freezes the
    window for as long as it takes. Reads are served from a ranked cache in
    `AppState` — the whole visible store, scored and ordered, rebuilt only when
    a generation counter or the reader's profile moves — because ranking 18,000
    items is not something to do three times per refresh. Anything that writes
    calls `invalidate()`. The list commands return `ListItem`, a borrowed
    projection of what a row renders; the fetched description, the score
    breakdown and `matched_skills` are not in it, and `get_item_detail(url)` is
    where the pane gets the rest. `cargo run --release --features dev-tools --bin perf_check` times
    the whole read path against the real database — measure before and after
    touching any of this.
  - `lib.rs` holds `run()` with `#[cfg_attr(mobile, tauri::mobile_entry_point)]`;
    `main.rs` is only a shim. **Mobile will not build if real code moves into
    `main.rs`.**
- **`app/src`** — the Svelte UI. Routes are `/` (feed), `/internships`, `/saved`,
  `/settings`, reached from a sidebar at `md:` and up and a bottom tab bar below
  it; the two navigations are never on screen together. `lib/feed.svelte.ts` is
  the single shared store; pages derive from it rather than fetching
  independently. **The three item lists are `$state.raw`**: the hub holds the
  whole opportunity cache, and plain `$state` deep-proxies every row and every
  nested array on assignment, which is seconds of frozen window per load. A raw
  list is a snapshot — replaced whole, never edited in place — so what *is*
  per-item and live (saved, seen, skill coverage) lives in reactive sets keyed
  by URL instead, read through `feed.isSaved` / `feed.isSeen` / `feed.match`.
  Never write to a field on an item and expect the UI to notice.
  - The feed and the saved list render through `ItemList`, which honours the
    card/list/compact `density` store and reveals results a page at a time.
  - `/internships` is a job-board split pane at `lg:` — result list beside a
    detail column — and falls back to the plain list plus `ItemModal` below
    that. `ItemDetail` is the shared body of the modal and the pane, so the two
    cannot drift apart.

See `app/README.md` for the source list, the command table, and Android setup.

## Conventions

- **No network calls from the webview.** All HTTP belongs in Rust. The one
  exception is the Google favicon service used for logos.
- **External links** open through `tauri-plugin-opener`, never by navigating the
  webview.
- **Item shape**: the Rust `Item` struct and the TS `ScrapedItem` interface must
  stay field-for-field identical. Changing one means changing the other. The
  trailing `matched_interests`/`saved`/`seen` fields are derived on read from
  `item_state` and the user's tags — they are never columns on the `items` row.
- **Adding a source**: implement `Scraper` in a new `scrapers/` module, register it
  in `all_scrapers()` under its category, and verify with
  `cargo run --features dev-tools --bin scraper_check`. Prefer a JSON or RSS endpoint over HTML
  scraping, and HTML over anything needing a browser — the app must never depend
  on a headless browser, which cannot exist on Android. Give every item a real
  timestamp; stamping `Utc::now()` on rows of unknown age makes the recency term
  hand that source the whole first page.
- **Per-source limits are measured, not guessed.** Check what the endpoint
  actually serves before picking a number: a feed carrying 32 entries gains
  nothing from a limit of 50, and one carrying 1,500 needs a cap for a reason.
  Where a source pages (Job Bank, Devpost), walk the pages and stop on the first
  empty or short one rather than firing a fixed number of requests. A failing
  page after the first ends the walk with what it has; a failing *first* page is
  a real error.
- **Never render a full result list.** The cache holds eighteen thousand items.
  Lists page through `InfiniteScroll`; a bare `{#each}` over a filtered feed is
  a visible stall on a phone.
- **Nothing per-item may be expensive.** A filter, a sort comparator or a search
  runs over the whole cache, so anything inside one is paid eighteen thousand
  times: build no strings you were not asked for (the hub's specialty haystack
  is built only when a specialty chip is on), key a sort on a value computed
  once per item rather than once per comparison, use a `Set` for membership, and
  construct `Intl` formatters at module scope.
- **Cross-platform care**: `reqwest` uses rustls + webpki-roots (not native-tls),
  and SQLite is bundled, both so the Android NDK targets cross-compile cleanly.
- **Tailwind 4 is CSS-first** — the semantic design tokens (`bg`, `surface`,
  `line`, `fg`, `muted`, `brand`, and the per-type accents) live in
  `app/src/app.css`, and there is no `tailwind.config.js`. Use the token
  utilities, not raw palette colours. Dark mode is a `dark` class applied before
  first paint by a script in `app.html`.
- **Icons come from `lib/icons.ts`**, which re-exports the lucide set the UI uses
  — no emoji in the interface, and no importing from `lucide-svelte` directly.
  Type an icon prop as `IconComponent` from that file; lucide still declares its
  icons as legacy `SvelteComponentTyped` classes, so a hand-written
  `Component<IconProps>` will not typecheck.

## Legacy

`/backend` (FastAPI + Python scrapers) and `/frontend` (Next.js) are the previous
two-process version, kept only as a porting reference. They are superseded and
should not receive new work. The root planning docs (`scraping_expansion_plan.md`,
`frontend_enhancement_plan.md`, `internship_filters_and_yorku_signin_plan.md`)
describe that older design; the eClass/YorkU SSO notes in the first are the part
still worth keeping for a possible future revival.
