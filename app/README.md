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
│   │   ├── filters.ts         filter facets shared by both feed pages
│   │   ├── icons.ts           the lucide icon vocabulary, re-exported
│   │   ├── item.ts            title/tag/CTA derivation for cards and the modal
│   │   ├── nav.ts             the four destinations, shared by both navs
│   │   ├── theme.svelte.ts    light/dark/system toggle
│   │   └── components/        Header, BottomNav, ItemCard, ItemModal,
│   │                          FilterSheet, CardSkeleton, EmptyState
│   └── routes/                / (feed), /internships, /saved, /settings
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

## How the data flows

On launch, and whenever the cache is more than 10 minutes old, all seventeen
scrapers run concurrently via `reqwest`. A failing source logs and yields
nothing rather than taking the refresh down with it. Results are deduplicated by
URL, classified by keyword, tagged with location facets, ranked, and written to
SQLite at the platform app-data directory. The UI reads only from that
database, which is why the feed works offline.

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
| Levels.fyi | public `internshipData.json` |
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
bump (currently `user_version` 3). Orphan rows are pruned after each refresh.

## Commands

| `invoke()` | Returns |
| --- | --- |
| `get_feed(major?)` | top 60 ranked items |
| `get_internships(major?)` | every job/internship, ranked |
| `get_saved()` | starred items, newest star first |
| `feed_status()` | last refresh, counts, staleness, per-source health |
| `refresh(force)` | items written, or `null` if the cache was fresh |
| `set_saved(item, saved)` | stars/unstars; the whole item is sent so it can be snapshotted |
| `set_dismissed(url, dismissed)` | hides or unhides one item |
| `mark_seen(url)` | records an open |
| `clear_dismissed()` | restores every hidden item |
| `list_interests()` / `get_interests()` / `set_interests(interests)` | the interest tags |
| `get_setting(key)` / `set_setting(key, value)` | local preferences |
| `list_sources()` | distinct sources currently cached |

## Development

```bash
npm run check                        # svelte-check
cargo test --lib                     # unit tests (location, ranking, db)
cargo run --bin scraper_check        # live per-source item counts
```

`scraper_check` is the tool to reach for when a source goes quiet — it prints
each scraper's item count and the first title, and exits non-zero if any source
returns nothing. It then runs the real `fetch_all` + `personalize` path and
reports the deduped type composition and how many distinct sources the top 20
spans. That last number is the one that catches a ranking regression: a feed can
have every scraper healthy and still put three repos on the whole first page.

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

## Notes

- **Linux/Wayland**: WebKitGTK's DMABUF renderer crashes the webview on some
  driver combinations, so the app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` at
  startup unless you have already set it yourself.
- **Fonts** are vendored into `static/fonts/` rather than fetched from Google,
  so the app renders correctly offline.
- **Favicons** come from Google's public favicon service and are the only
  outbound request the UI makes; both the card and the modal fall back to a
  letter avatar when it fails.
