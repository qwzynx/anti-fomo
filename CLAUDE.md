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
    take the whole refresh down. **A source hands over its rows in pieces**
    (`Scraper::fetch_batches`, default one piece from `fetch`; the ATS families
    override it) and `fetch_stream` sends them over a channel, so rows are stored
    while the scrape is still running. A piece is a promise — whatever was sent
    is written — and a board's rows always travel in one piece, because the sweep
    reasons per employer. **Stop = drop the producer** (`Abortable`), never a loop
    `break` returning `Ok(partial)`: for a list source that would charge misses to
    the rows it did not fetch. Writes happen in a consumer that is *not* abortable,
    so a stop cannot land in the middle of one.
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
  - Sources are `registry()` entries with a `Category` (News / Opportunities /
    Events): a kind of *source*, not an `ItemType`. `fetch_selected` runs a subset
    and reports each as it finishes. `Dormant` is the error a source returns when
    it answered correctly but its list is abandoned (Pitt CSC's repo, untouched
    for 74 days) — the schedule treats that as having worked, and the sweep must
    not be fed its rows, or dead listings would become permanent. Dedupe keeps the
    first copy of a URL but `absorb`s the deadline, pay and text the dropped copy
    carried. Where a cap must stay it keeps the best, not the newest:
    `github_internships::select`, `ats::keep_best`, and `rank::cap_opportunities`
    by `rank::fit` at ingest.
  - `scrapers/boards.rs` + `boards.json` — which employer boards the ATS scrapers
    read: the curated `EMPLOYERS` (tiered), a bundled list of probed boards, and
    boards learned from posting URLs in the cache (`discover`). Health is kept in
    `settings` (`boards_health`, `boards_discovered`), not a table, so it needs no
    schema bump and survives one. A board that is *definitively* gone (404/410) or
    empty on three separate days (strikes are spaced 24 h apart, like the sweep's
    misses) is skipped for a week. A timeout, a 429 or a 5xx never counts: three
    refreshes on a laptop with no network must not retire the whole list. A family that is one host behind many subdomains
    (Recruitee) rate-limits, so each family has its own concurrency
    (`ats::concurrency_for`) and `ats::get_json` waits out a 429. A tenant's JSON
    shape varies (Breezy's `location`, Recruitee's string salaries): read the
    fields that vary loosely rather than failing a board over one.
    `rank::worth_storing` is the ingest filter for sources that read whole boards:
    early-career in any field, technical at any level, judged on the *title* — a
    substring match on the body ("api" in "Capital One") calls everything
    technical. `OPPORTUNITY_CEILING` is 40,000, held by `perf_check` gates, not by
    taste.
  - `location.rs`, `rank.rs`, `skills.rs` — pure functions (location tagging,
    discipline classification, interest-tag matching, recency decay, relevance
    scoring, round-robin per-source diversification, skill extraction).
    Unit-tested. **A deadline is weighed against quality, not added to it.**
    `personalize_with` is two passes: every item gets its `merit` (all terms except
    recency, a deadline and `seen`), then an item with a deadline gains
    `w.urgency × urgency × Bar::gate(merit)` — so a weak posting closing tomorrow
    cannot outrank a better one closing later. `last_call` is merit above the bar
    (the top decile of what is being ranked, floored: an empty profile is a flat
    field and calls nothing really good) with the deadline inside a week. The bars
    are measured over the whole set and stored with the ranked cache, so rescoring
    a single item (`item_detail`) must pass them in — a percentile of one item
    means nothing. The cache is also rebuilt when the hour turns, because
    "closes in two days" is a statement about a clock. The detail queue is ordered
    by merit for the same reason: a deadline is only learned by fetching.
    **A posting's skills come from its own fetched text and
    nothing else** — its requirements, its duties, its description and the
    tags the source attached, read together. Read **line by line**: where a
    mention sits is most of what it means, so `skills::demand` returns each
    skill with an `Emphasis` (must / nice / mentioned) and a weight, a
    "Preferred qualifications:" heading governs the bullets under it, and a
    skill the posting only offers to *teach* is dropped rather than reported
    as an ask. `extract` is the names out of that, and is what
    `required_skills` still is; `key_skills` is the must-have subset.
    Matching is boundary-aware — a leading or trailing space in a catalog
    keyword means *word boundary*, not a literal space, which is what lets
    `" rust "` match "(Rust)" and "Rust/Go" while `+`/`#`/`&` count as word
    bytes so "C" stays out of "C++" and "R" out of "R&D". Nothing is inferred
    from the job title, so a posting whose page could not be read reports no
    skills at all; `skills::from_posting` says which case it is, and
    `hasScrapedPosting` in `lib/item.ts` must keep answering the same question,
    since that is what decides whether the UI shows a panel or explains its
    absence.
    `skills::from_text(text, strict)` is the same reader pointed at one string,
    and it is how a résumé bullet gets its skills — so a bullet and a job
    description are matched over one vocabulary rather than two that drift.
    Résumé callers pass `strict = true`, because a bullet is English prose
    where " go " is far more often the verb than the language.
  - `resume/` — the résumé builder and the tailored PDF. Layered so one thing
    knows about each concern: `model` (what a résumé is), `theme` (how it
    looks), `tailor` (what belongs on the page for a given posting), `terms`
    (what a posting is about, in its own words), `text` (how wide a string
    is), `layout` (where every box goes), `pdf`, and `jsonresume`.
    **A bullet is matched to a posting twice**: on the catalog skills they
    share, weighted by how hard the posting asked, and on how much of the
    posting's own vocabulary it speaks. `terms` is the second half — stemmed,
    bigrammed, boilerplate-stripped, weighted by section and repetition —
    because the catalog is a hundred and fifty technologies and a posting says
    far more, and a bullet about an on-call rotation names no catalog skill
    and is exactly what an SRE posting wants kept. The two are reported apart
    (`Selection::why` vs `Selection::echo`) so the weaker evidence cannot read
    as coverage. Tailoring also *orders*: Projects, Leadership and Awards lead
    with the entry that speaks to the posting, and Experience and Education
    never do — a reordered work history reads as a mistake. The reader can
    reorder anything by hand (drag in the tailor outline, `lib/sortable.ts`):
    `Variant::order` per section's entries and `Variant::section_order`,
    which `resume::tailored` applies before the pass — only the reader ever
    moves a section.
    **`layout` emits positioned boxes and has two consumers** —
    `pdf.rs` writes them into a PDF and `ResumeSheet.svelte` (one page, inside
    the paged `ResumePreview` viewer) draws the very
    same boxes as SVG. A résumé's whole job is fitting a page, so a preview
    laid out separately from the file would drift about where a line breaks;
    one pass, two backends, is what makes the preview *be* the file. Text is
    measured with the glyph advances of the same face printpdf embeds. Nothing
    is ever reworded — there is no model here and no API key. Verify with
    `cargo run --features dev-tools --bin resume_check`, which renders every
    theme and runs `pdftotext` over the output; printpdf's own `extract_text`
    replays ops rather than reading the file back and will happily report text
    that never reached the page.
    A job description pasted in from anywhere (`/resume/tailor` with no
    `?url=`, the sidebar's "Tailor résumé") is a `pasted_postings` row with
    the URL `pasted:<id>`; `item_detail` builds an `Item` from it through
    `sections::split`, so every résumé command works on it unchanged. Rows
    are never edited — the skill memo is keyed by URL. Empty text is a version
    made by hand with no job behind it: it tailors against nothing, so the
    reader's ticks and the page limit decide the page.
  - `prefs.rs` — what the reader asked to have scraped (`ScrapePrefs`) and which
    sources that makes due. Pure, so the rules that decide what costs a request
    are tested without an app: `due` (a source never run, or older than its
    category's window, or 15 minutes after a failure), `made_due` (what a change
    of mind refetches) and `admits` (internships vs jobs, decided on the *row*,
    since every ATS board emits `Job`). **Nothing is fetched before the setup form
    is answered**: `commands::refresh` returns early without `onboarded_at`, which
    `db::migrate` backfills for an existing install (`user_version > 0`) and
    `complete_onboarding` writes for a new one. The launch refresh and every button
    go through that one gate.
  - `db.rs` — SQLite via `rusqlite` (bundled). Items are a rebuildable cache keyed
    by URL. `settings`, `item_state`, `job_details`, `resumes`,
    `resume_variants` and `pasted_postings` are the durable tables and must survive a schema bump, so
    they are created with `IF NOT EXISTS` and never dropped. **Additive schema
    goes in `open()`, not `migrate`**: a version bump drops the items cache and
    costs a full re-scrape, so new durable columns (`ensure_application_columns`
    — the tracker's `app_status`/`app_updated_at`/`app_note` on `item_state`)
    and the description index (`ensure_text_index`) are idempotent steps run on
    every open. `job_text` is a contentless FTS5 index over `job_details`, kept
    in step by triggers and keyed through `job_text_ids` (an `INTEGER PRIMARY
    KEY`, because `VACUUM` may renumber a TEXT-keyed table's rowids); inside a
    trigger the outer upsert's conflict policy overrides `OR IGNORE`, so guard
    inserts with `NOT EXISTS`. `search_details` answers per term so the UI can
    AND terms across a row's own fields and its description. `clear_data`
    empties the cache and leaves the résumés: a feed can be refetched and a
    work history cannot. **Deleting rows does not shrink the file.** Freed
    pages go on a freelist and are reused, so without help the store sits at
    its own high-water mark forever and the retention sweep returns nothing to the
    disk. `open` puts the file in incremental auto-vacuum — converting one that
    predates that, non-fatally, since the machine where a `VACUUM` cannot run
    is the full one that most needs the app to still launch — and
    `incremental_vacuum` runs after the prunes in `persist`. That pragma moves
    **one page per step**, so it must be stepped to exhaustion; `execute_batch`
    steps once and reclaims 4 KB while looking like it worked.
    `pruning_hands_the_pages_back_to_the_disk` asserts on the size of the file
    and the freelist, which is the only thing that tells the two apart.
    `journal_size_limit` is the counterpart for the WAL, which otherwise keeps
    the width of the largest refresh long after that refresh is over.
    **Writes are per piece, the sweep is once.** `persist_batch` upserts each piece
    (`save_items_ranked`) and never sweeps; `finalize` runs once at the end, stopped
    or not. Sweeping per source would delete a posting two sources list the moment
    the first one stopped listing it, before the second had been written — and take
    its dismissed state and fetched details with it. A stopped run passes an empty
    alive set, so only the calendar deletes anything. Which source *owns* a URL is
    decided in the upsert (earlier run, or earlier in `registry()` order), not by
    arrival order. **Retention is liveness, not age.** `sweep` runs in `persist` after
    `save_items_at` and drops a posting when its source stops offering it (three
    scrapes that vouched for its scope without returning it, at least 24 h apart),
    when its deadline passed a week ago (`job_details.closes_at` first — most
    deadlines live there), or when its event ended; only news ages out by date.
    It used to be a blanket 60 days on `timestamp`, which deleted the oldest
    postings still open and, for Devpost (stamped with the day entries *opened*),
    a hackathon the day it was saved. The scope is (source, **employer**) for the
    ATS sources, because `ats::fan_out` swallows a failed board and the source
    still looks healthy: sweeping by source would read one dead board as every
    posting on it gone. A scope that returned nothing is never charged. `source_runs`
    (dropped with `items`, emptied by `clear_data`) is the per-source ledger that
    the schedule and Settings read; it must not outlive an emptied cache or every
    source would read as fresh over nothing. The policy table is
    `scrapers::liveness`, keyed on `source_platform` strings that
    `the_policy_table_names_real_sources` pins.
  - **Locks go through `sync::LockExt`** (`lock_recover`, `read_recover`,
    `write_recover`), never `.lock().unwrap()`. Release builds unwind so that
    `scrapers::stream_sources` and the detail pass can `catch_unwind` a source
    or page that panics; a poisoned lock must not then brick every command.
    `set_setting` does not re-rank for `ui.`-prefixed keys (the Roles filters,
    saved searches), which are written on every chip tap.
  - `commands.rs` — the `invoke()` surface the UI calls. **Every command that
    touches the database is `#[tauri::command(async)]`.** A plain
    `#[tauri::command]` on a synchronous function runs on the *main thread*,
    which is the webview's thread, so a read that walks the cache freezes the
    window for as long as it takes. Reads are served from a ranked cache in
    `AppState` (`Freshness`: list reads accept a cache a few seconds old *while a
    scrape is running*, the detail pane and the description queue accept any age —
    a ranking pass per streamed piece would otherwise sit behind every click) — the whole visible store, scored and ordered, rebuilt only when
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
  `/resume`, `/resume/tailor` and `/settings`. **It is an app shell, not a
  page**: the root layout is exactly the window (`h-dvh`), with `Sidebar` at
  `md:` and up (views with counts, Applications = `/saved?stage=active`, saved
  searches, tools, sync status) and the bottom tab bar below it — the two are
  never on screen together. Every route is a column: `PageHeader` is a
  one-line toolbar (title, count, an optional middle slot such as a search
  field, actions), and beneath it panes that scroll on their own (the `pane`
  utility). Nothing scrolls the window and there is no footer. Build new views
  from `toolbar`/`pane`/`tool`/`btn-primary`/`eyebrow`/`kbd` in `app.css`, at
  13px UI text. `lib/feed.svelte.ts` is
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
  - The setup form (`Onboarding.svelte`) is a full-screen gate, deliberately not
    a `Modal` (a stray Escape must not strand an empty first-run feed). The store's
    `onboarding` is `unknown | needed | offered | done` so a returning reader never
    sees it flash; `feed.scrapeNow()` calls Rust directly rather than `refresh()`,
    which gives up if one is already running, and resolves when the *run* ends,
    not when the feed is usable: the form offers "Open Today" as soon as rows exist.
    While a scrape runs the store reloads single-flight at most every three seconds
    (an event during a load marks it dirty; a sequence token drops a stale load),
    with the spacing growing with how long a load took (at forty thousand roles the
    twenty-odd-megabyte payload parsed on the window's thread *is* the stutter), and
    holds the update behind a "Show new results" pill when a list is scrolled
    away from its top. `ScanBar` is the one place a running scrape is visible and
    stoppable from every route. The seniority and region choices come
    from `list_setup_options`, not a copy: they are matched by equality and a typo
    matches nothing. Today's "Last call" band is its own list (`get_last_call`),
    since a strong posting closing tomorrow can rank past the feed's 400.
  - `/internships` is a job-board split pane at `lg:` — result list beside a
    detail column — and falls back to the plain list plus `ItemModal` below
    that. `ItemDetail` is the shared body of the modal and the pane, so the two
    cannot drift apart. Roles has its own search field as well as the ⌘K palette;
    both use `lib/query.ts` (AND-ed word-start terms, `"whole words"`,
    `-exclude`, `company:`), indexed once per list snapshot, plus Rust's
    description hits. Its filter state lives in `lib/roles.svelte.ts`, outside
    the page, persisted as `ui.roles_filters`; `filterRoles` in `lib/filters.ts`
    is the one pass, shared with saved-search counts.
  - **Feedback for an action is a toast** (`lib/toast.svelte.ts`), with an Undo
    where the action can be taken back. `feed.error` and its banner are only
    for a refresh that failed. Destructive actions ask through
    `confirm()` (`lib/confirm.svelte.ts`). Overlays register with
    `lib/layers.svelte.ts` so Escape closes only the top one and the scroll
    lock is counted. Uncaught UI errors go to the log file via `lib/errors.ts`.

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
- **`target/` is not free.** It reached 23 GB once and filled a disk. The dev
  profile builds dependencies with no debug info at all and this crate with
  `line-tables-only` — a panic backtrace still names the file and line, and
  nothing here is debugged through breakpoints in `gtk`. `[lib]` deliberately
  builds no `staticlib`: only iOS links one, `gen/` holds `android` alone, and
  the archive nobody opened cost 1.3 GB on the host, 0.6 GB on the Android
  target and a staging copy of its own size on every link. Put `"staticlib"`
  back the day an `apple` target appears under `gen/`.
- **Tailwind 4 is CSS-first** — the semantic design tokens (`bg`, `surface`,
  `line`, `fg`, `muted`, `brand`, and the per-type accents) live in
  `app/src/app.css`, and there is no `tailwind.config.js`. Use the token
  utilities, not raw palette colours. Dark mode is a `dark` class applied before
  first paint by a script in `app.html`.
- **The résumé faces are read twice from one file.** The eight `.ttf`s in
  `app/static/fonts/resume/` are `include_bytes!`d into the Rust binary so the
  PDF can embed them *and* `@font-face`d in `app.css` so the preview draws with
  the metrics the PDF was measured against. One copy on disk is the guarantee
  those two agree; replacing either side with a `.woff2` breaks it silently, as
  a preview that wraps one word earlier than the page it claims to be.
  Subsetting is an authoring step (`scripts/subset-resume-fonts.sh`), because
  printpdf's runtime subsetter is a no-op without its `text_layout` feature and
  that feature drags in a layout and fontconfig stack that cannot cross-compile
  to Android.
- **Gates**: `npm run check`, `npm test` (vitest, pure modules in
  `src/**/*.test.ts`), `cargo fmt --check`, `cargo clippy --all-targets
  --features dev-tools -- -D warnings`, `cargo test --lib`. CI runs all five.
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
