# Anti-FOMO

Internships, tech news and events from ten public sources, scraped on your own
device and ranked into one feed. Runs on Linux, Windows and Android from a
single Rust + Svelte codebase.

There is no server, no account and no hosting bill. The scraping, ranking and
storage all happen inside the app.

## Layout

```
app/
├── src/                       Svelte 5 + Tailwind 4 UI
│   ├── lib/
│   │   ├── api.ts             invoke() wrappers around the Rust commands
│   │   ├── feed.svelte.ts     shared feed store (items, status, refresh)
│   │   ├── filters.ts         filter facets shared by both feed pages
│   │   ├── item.ts            title/tag/CTA derivation for cards and the modal
│   │   ├── theme.svelte.ts    light/dark/system toggle
│   │   └── components/        Header, ItemCard, ItemModal, FilterSheet
│   └── routes/                / (feed), /internships, /settings
└── src-tauri/
    ├── src/
    │   ├── scrapers/          one module per source + the concurrent runner
    │   ├── location.rs        raw location strings → modality/city/region tags
    │   ├── rank.rs            discipline classification + relevance scoring
    │   ├── db.rs              SQLite cache and settings
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

On launch, and whenever the cache is more than 10 minutes old, all ten scrapers
run concurrently via `reqwest`. A failing source logs and yields nothing rather
than taking the refresh down with it. Results are deduplicated by URL,
classified by keyword, tagged with location facets, ranked, and written to
SQLite at the platform app-data directory. The UI reads only from that
database, which is why the feed works offline.

| Source | Technique |
| --- | --- |
| Hacker News | Algolia JSON API |
| Pitt CSC Repo | HTML tables in the repo README |
| Simplify | HTML tables in the repo README |
| Levels.fyi | public `internshipData.json` |
| Luma | `__NEXT_DATA__` blob on the Toronto city page |
| TLDR Tech | latest issue discovered from the archive index |
| Phoronix | RSS |
| Lassonde News | RSS (needs a browser User-Agent or it 403s) |
| HN Top Links | HTML |
| Daily.dev | public GraphQL `tagFeed` |

## Commands

| `invoke()` | Returns |
| --- | --- |
| `get_feed(major?)` | top 60 ranked items |
| `get_internships(major?)` | every job/internship, ranked |
| `feed_status()` | last refresh, cached count, staleness |
| `refresh(force)` | items written, or `null` if the cache was fresh |
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
returns nothing.

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
