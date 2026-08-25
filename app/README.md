# Anti-FOMO

Internships, tech news and events from seventeen public sources, scraped on your
own device and ranked into one feed. Runs on Linux, Windows and Android from a
single Rust + Svelte codebase.

There is no server, no account and no hosting bill. The scraping, ranking and
storage all happen inside the app.

## Layout

```
app/
├── src/                       Svelte 5 + Tailwind 4 UI
│   ├── app.css                the Tailwind 4 @theme token palette
│   ├── lib/
│   │   ├── api.ts             invoke() wrappers around the Rust commands
│   │   ├── feed.svelte.ts     shared store (feed, saved, interests, status)
│   │   ├── filters.ts         the facets behind the Roles chip bar
│   │   ├── icons.ts           the lucide icon vocabulary, re-exported
│   │   ├── item.ts            title/tag/CTA derivation for rows and the detail
│   │   ├── nav.ts             three destinations + Settings as a tool
│   │   ├── search.svelte.ts   the one search surface, opened from the top bar
│   │   ├── theme.svelte.ts    light/dark/system toggle
│   │   └── components/        TopBar, BottomNav, PageHeader, Band, HeroMatch,
│   │                          ItemRow, ItemDetail, ItemModal, SearchPalette,
│   │                          InfiniteScroll, RowSkeleton, EmptyState
│   └── routes/                / (Today), /internships (Roles), /saved, /settings
└── src-tauri/
    ├── src/
    │   ├── scrapers/          one module per source + the concurrent runner
    │   ├── location.rs        raw location strings → modality/city/region tags
    │   ├── rank.rs            classification, scoring, source diversification
    │   ├── db.rs              SQLite cache, settings, saved/dismissed/seen
    │   ├── commands.rs        the invoke() surface
    │   └── lib.rs             Tauri setup, background refresh on launch
    └── tauri.conf.json
```

## Running it

```bash
npm install
npm run tauri dev
```

The window opens immediately on whatever is cached and refreshes in the
background, so a cold start never blocks on the network.

## Interface

Three destinations — Today, Roles, Saved — and Settings as a tool rather than a
fourth. At `md:` and up they sit in a 58px top bar as a segmented control, with
global search in the middle and the sync state and Settings on the right. Below
`md:` the top bar is not rendered at all: a bottom tab bar owns navigation (with
Settings as a fourth tab, since a phone has nowhere else to put it) and each
page carries its own title row. The two navigations are never on screen
together.

`/` is Today, not a feed. One hero band states the best-ranked role you have not
seen — with the skill-coverage meter beside it — and below that, three bands of
three: Roles, Reading, Happening. There are no filters on it. Bands reveal
twelve more at a time rather than the whole cache.

Search is app chrome, reached from the top bar or ⌘K, and opens over whatever
page you are on. There are no per-page search fields.

Roles is the only filter surface. Everything is a chip: what is on is filled and
carries a cross, what is off is an outline, and "More" adds facets to the same
bar rather than hiding the current state in a drawer. At `lg:` the result list
sits beside a detail column that stays put and opens on the top-ranked role, so
comparing two postings costs a glance. Narrower than that the list goes full
width and the detail opens as a sheet — both render `ItemDetail`, so the two
cannot drift apart.

Every list pages as you scroll, so a filtered list of several hundred never has
to render at once. Save and dismiss are always visible on a row, never revealed
on hover: there is no hover on Android.

## How the data flows

On launch, and whenever the cache is more than 10 minutes old, all seventeen
scrapers run concurrently via `reqwest`. A failing source logs and yields
nothing rather than taking the refresh down with it. Results are deduplicated by
URL, classified by keyword, tagged with location facets, ranked, and written to
SQLite at the platform app-data directory. The UI reads only from that
database, which is why the feed works offline.

A full refresh lands roughly 1,650 items, about 1,150 of them jobs and
internships — the category the app exists for. Per-source limits are set against
what each endpoint actually serves rather than guessed, and the sources that
page (Job Bank, Devpost) are walked until they run dry.

**News**

| Source | Technique |
| --- | --- |
| Hacker News | Algolia JSON API |
| HN Top Links | HTML |
| Phoronix | RSS |
| Lobsters | RSS |
| Ars Technica | RSS |
| The Verge | Atom |
| InfoQ | RSS |
| Lassonde News | RSS (needs a browser User-Agent or it 403s) |
| TLDR Tech | latest issue discovered from the archive index |
| Daily.dev | public GraphQL `tagFeed` |

**Opportunities**

| Source | Technique |
| --- | --- |
| Pitt CSC Repo | HTML tables in the repo README |
| Simplify | HTML tables in the repo README |
| New Grad Positions | HTML tables in the repo README (full-time, not interns) |
| Job Bank Canada | HTML search results from the federal job board |

**Events**

| Source | Technique |
| --- | --- |
| Luma | `__NEXT_DATA__` blob on the Toronto, SF and NYC city pages |
| Devpost | public `/api/hackathons` JSON |

## Ranking

`rank.rs` scores every item, then interleaves the result so no single source can
own the first page:

- **discipline match** with the chosen major, +10
- **kind**: opportunities +5, events +4 (news is the baseline)
- **interest tags** the item matched, +4 each, capped at +8
- **recency**, up to +6 on a 48-hour half-life — the decay is applied to the
  absolute distance from now, so a hackathon three months out ranks below one
  next week
- **already seen**, −3

`diversify()` then buckets the scored items by source and pops them round-robin,
best-front-item first. The three big internship repos post hundreds of rows a
day; without this they crowd out everything else.

The twelve interest tags (AI/ML, Frontend, Backend, Systems, Security, Data,
DevOps/Cloud, Mobile, Hardware, Product/Design, Game Dev, Startups) are picked
in Settings and stored in `settings`. A card shows which of them it matched.

## Saved, dismissed and seen

`items` is a rebuildable cache — a refresh drops and rewrites it — so the user's
own marks live in a separate durable `item_state` table keyed by URL:

- **saved** stars an item onto `/saved`. The row also holds a JSON `snapshot` of
  the item, so a saved listing survives falling out of the cache.
- **dismissed** hides an item from every list permanently. Settings can restore
  them all.
- **seen** dims an already-opened card and applies the −3 ranking penalty.

`item_state` is created with `IF NOT EXISTS` and is never dropped by a schema
bump (currently `user_version` 6). Orphan rows are pruned after each refresh.

## Job descriptions

Every opportunity source reads a *list* endpoint, so a scraped posting arrives
with 24–70 characters of `content_text` that is mostly the office location.
What the employer requires, what the job involves and what it pays for are on
the posting's own page and nowhere else.

So a refresh has a **second phase**. After the scrape is persisted, up to 200
postings that have never been tried get their real description fetched, eight
at a time, and the UI is told to re-rank when the batch lands.
`scrapers::details::route` returns an ordered *chain* per posting and
`fetch` walks it until something comes back with text — the employer's own
words first, a mirror last:

1. **The ATS's own JSON API**, where the posting link names one we handle:
   Workday (`/wday/cxs/…`, 31% of a real cache on its own), SmartRecruiters,
   Greenhouse, Lever, Workable, Eightfold, plus Job Bank Canada's own pages.
   Lever, SmartRecruiters and Workable label their sections themselves, which
   beats any heading guess.
2. **schema.org `JobPosting` metadata** on the posting's page. Every ATS wants
   its listings in Google's job search, so the pages that hand a browser
   nothing but a JavaScript shell still hand us the full description in a
   documented shape. This is what serves the iCIMS family, Ashby, Microsoft
   and the long tail of employer career sites.
3. **simplify.jobs**, for the rows the three GitHub repos gave a posting `id`.
   A mirror of the same posting, so it goes last — but a good one, and it also
   carries tagged `skills`, which nothing else does.

A handler that returns only unsegmented prose is not the end of the walk: if
simplify.jobs is still in the chain it is tried too and merged in, since its
pre-split requirements are what the prose was missing.

Descriptions arrive as one HTML blob from most of these, so
`scrapers::sections` recovers the Requirements / Responsibilities / Perks split
from the employer's own headings — `<h3>`, a fully bold paragraph, a line
ending in a colon and a short bare line sitting directly on top of a list all
count, because whoever pasted the description into the ATS used whichever they
felt like. It is the only module that knows those heading words. Every handler
goes through it, simplify.jobs included: stripping a description to plain text
instead loses the structure the UI renders *and* the headings that separate
requirements from the company culture blurb.

Measured over a 1,918-item cache with 1,438 opportunities: **99% routable**,
and a live sample of 200 serves **95%** of what it routes — 82% with
requirements, 74% with responsibilities, 58% with perks, at 56 ms per posting.
Over the same sample, skills read off the posting go from a mean of 0.0 — the
scraped line names almost nothing — to **4.9**, and the share with nothing to
show falls from 96% to 24%.

Results live in a durable `job_details` table, **not** on the `items` row:
`save_items` overwrites `content_text` on every refresh, so a description
stored there would be destroyed by the next scrape whose fetch happened to
fail. That table is also the ledger that keeps enrichment incremental — a row
means "already attempted", including a failure, so a dead link costs one
request rather than one per refresh forever.

Deliberately not attempted: a readability-style "grab the biggest block of
text" fallback. Garbage text feeds garbage into skill matching, and a posting
with no skills listed is a better answer than one credited with the words in a
site's navigation menu.

The postings that still come back with nothing — an expired listing, a page
behind a login — show **no skills at all**, and the detail pane says why. There
used to be a `skills::ROLES` table that guessed a toolset from the job title
for exactly those; it is gone. A guess and a reading look identical once they
are chips in the same panel, so a match score built on one is not something the
reader can act on: "this posting asks for React" has to mean the posting said
React.

What is read is the requirements, the duties and the description together —
not the requirements alone, because a posting that names its stack once in the
opening paragraph and never repeats it in a bullet is ordinary. `perks` is
excluded: a benefits list says what the job pays, not what it asks for.

`cargo run --features dev-tools --bin detail_check` reports routing and live coverage per handler,
and what enrichment does to skill extraction, against the real cached database.

## Commands

Every command that touches the database is `#[tauri::command(async)]`. A plain
`#[tauri::command]` on a synchronous function runs **on the main thread**, which
is the webview's thread — a read that walks the cache freezes the window for as
long as it takes.

The two list commands return `ListItem`, not `Item`: what a row renders, and
nothing else. No fetched description, no score breakdown, no `matched_skills`.
`get_item_detail` is where the rest of a posting comes from, and it is called
for the one posting the pane is showing.

| `invoke()` | Returns |
| --- | --- |
| `get_feed(major?)` | top 400 ranked list rows |
| `get_internships(major?)` | every job/internship as a list row, ranked |
| `get_item_detail(url)` | one posting in full — description, sections, score breakdown |
| `get_saved()` | starred items, newest star first |
| `feed_status()` | last refresh, counts, staleness, per-source health |
| `refresh(force)` | items written, or `null` if the cache was fresh |
| `set_saved(item, saved)` | stars/unstars; the whole item is sent so it can be snapshotted |
| `set_dismissed(url, dismissed)` | hides or unhides one item |
| `mark_seen(url)` | records an open |
| `clear_dismissed()` | restores every hidden item |
| `list_interests()` / `get_interests()` / `set_interests(interests)` | the interest tags |
| `list_skills()` | the skill catalog, grouped into categories |
| `get_skills()` / `set_skills(skills)` | the skills the user says they have |
| `get_setting(key)` / `set_setting(key, value)` | local preferences |
| `list_sources()` | distinct sources currently cached |
| `clear_data()` | empties the item cache; the profile and résumés are kept |
| `list_resumes()` | the résumé picker's rows |
| `get_resume(id?)` | one résumé; no id means the default one |
| `save_resume(id?, name, doc, theme)` | creates or updates, returns the id |
| `delete_resume(id)` / `set_default_resume(id)` | — |
| `get_resume_variant(url, resumeId)` | one posting's overrides, if any |
| `save_resume_variant(...)` / `clear_resume_variant(...)` | write / discard them |
| `layout_resume(id?, url?, theme?)` | positioned boxes, plus the tailoring with a `url` |
| `render_resume_pdf(id?, url?, theme?)` | the PDF, as raw bytes |
| `import_json_resume(json, name?)` / `export_json_resume(id?)` | the JSON Resume format |
| `list_resume_themes()` | the four versions, named and pre-coloured |

## Development

```bash
npm run check                        # svelte-check
cargo test --lib                     # unit tests (location, ranking, résumé layout, db)
cargo run --features dev-tools --bin scraper_check        # live per-source item counts
cargo run --features dev-tools --bin resume_check         # résumé PDFs, incl. text extraction
cargo run --release --features dev-tools --bin perf_check -- /path/to/copy.db
```

`perf_check` times every stage of a feed read against the real database — the
load, the ranking pass cold and warm, and the size of what crosses the IPC
boundary. Run it against a **copy**: it opens the file directly rather than
through `db::open`, which would migrate and drop `items`. Reach for it before
changing anything on the read path, and again afterwards; the numbers in the
comments around `rank`, `skills` and `commands` all came from it.

`scraper_check` is the tool to reach for when a source goes quiet — it prints
each scraper's item count and the first title, and exits non-zero if any source
returns nothing. It then runs the real `fetch_all` + `personalize` path and
reports the deduped type composition and how many distinct sources the top 20
spans. That last number is the one that catches a ranking regression: a feed can
have every scraper healthy and still put three repos on the whole first page.

`resume_check` renders the fixture résumé in every version and both page sizes,
checks nothing lands outside the margins, and — the part that matters — runs
`pdftotext` over each file and asserts the name, the headings and every bullet
come back out. An applicant tracking system reads a résumé with something very
like it, and a PDF that renders perfectly and extracts as nothing is worse than
no feature at all. It deliberately does **not** use printpdf's own
`extract_text`, which replays the ops it was handed rather than parsing the file
back: an early revision positioned text with `Td` (a *relative* move) instead of
`Tm`, so every line after the first compounded its offset and the third ran off
the page — `extract_text` cheerfully reported all three, `pdftotext` saw one.
Install poppler for it; without it the geometry checks still run and the
extraction check reports itself skipped rather than passing quietly.

## Résumés

A résumé the user writes once, and a PDF tailored to whichever posting they are
looking at. Everything is in `src-tauri/src/resume/`, layered so one thing knows
about each concern:

```text
  model    what a résumé is            (no styling, no layout)
  theme    how it should look          (no content)
  tailor   what belongs on this page   (pure; uses layout to test the fit)
  text     how wide a string is        (pure; the font's own advances)
  layout   where every box goes        (pure; emits positioned boxes)
     ├── pdf      boxes → a file, via printpdf
     └── preview  boxes → SVG, in ResumePreview.svelte
  jsonresume  import/export against the open schema
```

**`layout` having two consumers is the load-bearing decision.** A résumé's whole
job is fitting a page, so a preview that disagrees with the file about where a
line breaks is worse than no preview. Rust wraps the text using the real glyph
advances out of the same face printpdf embeds, resolves every alignment to a
left `x`, and emits positioned boxes; the PDF writer and the webview both just
draw them. The preview is SVG rather than HTML because `<text y>` puts the
baseline exactly at `y`, which is what PDF's `Tm` does too — HTML would mean
reproducing CSS line-box maths to work out where a baseline lands.

The tailoring is the existing skill match pointed at the user's own words. Each
bullet is read by `skills::from_text`, the same catalog and automaton that reads
a job description, so matching a bullet to a posting is a set intersection over
one vocabulary. Rust starts from the *whole* résumé and trims: lay out, and
while it runs over the page budget drop the least valuable bullet and lay out
again. Pinned bullets are never trimmed, an explicit exclusion always wins, and
whatever came off is reported so the UI can show the trim rather than let it
look like data loss. Nothing is ever reworded — there is no model here and no
API key, and a feature that silently rewrote somebody's job history would be a
liability.

The faces are vendored in `static/fonts/resume/` and read twice: `include_bytes!`
into the Rust binary so the PDF can embed them, and `@font-face` in `app.css` so
the preview draws with the metrics the PDF was measured against. One file is
what makes that guarantee; do not replace either side with a `.woff2`.
`scripts/subset-resume-fonts.sh` regenerates them — subsetting is an authoring
step because printpdf's runtime subsetter is a no-op without its `text_layout`
feature, and that feature drags in a layout and fontconfig stack that has no
business on Android. Unsubsetted it mattered: one page of text came out a 397 KB
PDF, against 63 KB now.

`resumes` and `resume_variants` are durable tables. `clear_data` empties the item
cache and leaves them alone — the cache can be refetched and nothing anywhere
can rebuild somebody's work history.

## Android

The SDK and NDK install outside the repo:

```bash
./scripts/setup-android.sh           # from the repo root
source scripts/android-env.sh
npm run tauri android init
npm run tauri android dev            # with a device attached over adb
```

`src-tauri/gen/android/` is generated and gitignored — re-run `android init`
after a fresh clone. Tauri already puts `android.permission.INTERNET` in the
manifest, so no manual edit is needed; without it every scraper would silently
return empty.

## Releasing

Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds Linux,
Windows, macOS (a single universal binary) and Android in parallel. Each job
only uploads its bundles as workflow artifacts; a final `release` job collects
all four and creates **one** draft GitHub release named after the tag. Nothing
is published automatically — a draft is visible only to maintainers until one is
edited and published by hand.

Because the release is created once, at the end, a failure on any single
platform means no release is created at all rather than a half-filled one. Fix
the cause and re-run the workflow from the Actions tab: `workflow_dispatch`
takes the tag to build as an input, so a manual run does not depend on which
branch you launch it from.

Bump the version in all three of `app/package.json`, `app/src-tauri/Cargo.toml`
and `app/src-tauri/tauri.conf.json` before tagging — `tauri.conf.json` is what
names the bundles and sets the Android `versionCode`, and a tag that disagrees
with it produces assets labelled with the old version.

The assets on a release are:

| Platform | Assets |
| --- | --- |
| Linux | `.deb`, `.rpm`, `.AppImage` |
| Windows | `.msi`, `-setup.exe` (NSIS) |
| macOS | `.dmg` (universal) |
| Android | `.apk`, `.aab` |

macOS ships unsigned/ad-hoc: there is no Apple Developer account behind this
build, so first launch needs a right-click → Open to clear Gatekeeper. Windows
is likewise unsigned, so SmartScreen may warn on first run.

Android is signed, so the APK/AAB in the release are directly installable. The
signing key lives outside the repo as three GitHub Actions secrets:

- `ANDROID_KEYSTORE_BASE64` — a release keystore, base64-encoded
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD` — used as both the store and key password

Generate the keystore once and keep it somewhere safe outside the repo — losing
it means every future release is a different signing identity, which Android
treats as a different app for update purposes:

```bash
keytool -genkeypair -v -keystore upload-keystore.jks \
  -keyalg RSA -keysize 2048 -validity 10000 -alias upload
# when prompted for the key password, reuse the store password
base64 -w0 upload-keystore.jks | pbcopy   # or: xclip -selection clipboard
```

Set the three secrets under the repo's Settings → Secrets and variables →
Actions. Without them the Android job still builds — its "Write release
keystore" step is skipped — but the resulting APK/AAB are unsigned and won't
install as-is.

`scripts/patch-android-signing.mjs` wires the keystore into
`gen/android/app/build.gradle.kts`, since that file is regenerated by `android
init` on every run and can't hold a hand edit permanently. It is a no-op when
`keystore.properties` is absent, so a local `tauri android build` still works.

## Notes

- **Linux/Wayland**: WebKitGTK's DMABUF renderer crashes the webview on some
  driver combinations, so the app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` at
  startup unless you have already set it yourself.
- **Fonts** are vendored into `static/fonts/` rather than fetched from Google,
  so the app renders correctly offline.
- **Favicons** come from Google's public favicon service and are the only
  outbound request the UI makes; both the card and the modal fall back to a
  letter avatar when it fails.
