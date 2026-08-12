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
  - `location.rs`, `rank.rs` — pure functions (location tagging, discipline
    classification, relevance scoring, per-source diversification). Unit-tested.
  - `db.rs` — SQLite via `rusqlite` (bundled). Items are a rebuildable cache keyed
    by URL; `settings` is the only durable table.
  - `commands.rs` — the `invoke()` surface the UI calls.
  - `lib.rs` holds `run()` with `#[cfg_attr(mobile, tauri::mobile_entry_point)]`;
    `main.rs` is only a shim. **Mobile will not build if real code moves into
    `main.rs`.**
- **`app/src`** — the Svelte UI. Routes are `/` (feed), `/internships`, `/settings`.
  `lib/feed.svelte.ts` is the single shared store; pages derive from it rather
  than fetching independently.

See `app/README.md` for the source list, the command table, and Android setup.

## Conventions

- **No network calls from the webview.** All HTTP belongs in Rust. The one
  exception is the Google favicon service used for logos.
- **External links** open through `tauri-plugin-opener`, never by navigating the
  webview.
- **Item shape**: the Rust `Item` struct and the TS `ScrapedItem` interface must
  stay field-for-field identical. Changing one means changing the other.
- **Adding a source**: implement `Scraper` in a new `scrapers/` module, register it
  in `all_scrapers()`, and verify with `cargo run --bin scraper_check`. Prefer a
  JSON or RSS endpoint over HTML scraping, and HTML over anything needing a
  browser — the app must never depend on a headless browser, which cannot exist
  on Android.
- **Cross-platform care**: `reqwest` uses rustls + webpki-roots (not native-tls),
  and SQLite is bundled, both so the Android NDK targets cross-compile cleanly.
- **Tailwind 4 is CSS-first** — the design tokens live in `app/src/app.css`, and
  there is no `tailwind.config.js`. Dark mode is a `dark` class applied before
  first paint by a script in `app.html`.

## Legacy

`/backend` (FastAPI + Python scrapers) and `/frontend` (Next.js) are the previous
two-process version, kept only as a porting reference. They are superseded and
should not receive new work. The root planning docs (`scraping_expansion_plan.md`,
`frontend_enhancement_plan.md`, `internship_filters_and_yorku_signin_plan.md`)
describe that older design; the eClass/YorkU SSO notes in the first are the part
still worth keeping for a possible future revival.
