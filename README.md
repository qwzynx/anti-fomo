# Anti-FOMO

<div align="center">

![Tauri](https://img.shields.io/badge/Tauri_2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)
![Svelte](https://img.shields.io/badge/Svelte_5-FF3E00?style=for-the-badge&logo=svelte&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS_4-38B2AC?style=for-the-badge&logo=tailwind-css&logoColor=white)
![SQLite](https://img.shields.io/badge/SQLite-003B57?style=for-the-badge&logo=sqlite&logoColor=white)

**Internships, tech news and events from ten sources, in one ranked feed.**

Linux · Windows · Android — one codebase, no server.

</div>

---

## About

Job postings live in GitHub internship-tracker repos, tech news lives in
newsletters and link aggregators, and events live somewhere else again — that is
a lot of tabs to check every day. Anti-FOMO scrapes all of them concurrently,
dedupes and tags the results, ranks them against your field, and shows one feed.

It is a **desktop and mobile app**, not a web service. The scraping, ranking and
storage all happen on your device in Rust; the Svelte UI reads from a local
SQLite database. There is nothing to deploy, no account to create, and the feed
still works with the network off.

## Getting started

```bash
cd app
npm install
npm run tauri dev
```

That is the whole setup. See [`app/README.md`](app/README.md) for the source
list, the command surface, the development commands, and Android builds.

## What it does

- **Ten sources**, all plain HTTP: Hacker News, Pitt CSC, Simplify, Levels.fyi,
  Luma, TLDR Tech, Phoronix, Lassonde News, HN Top Links and Daily.dev. A source
  that breaks degrades quietly instead of taking the refresh down.
- **Ranking** by discipline match and item type, with a per-source cap so one
  large internship repo cannot flood the feed.
- **Location tagging** that turns raw strings like `Toronto, ON<br>Remote` into
  filterable modality, city and region facets.
- **An internships hub** with specialty, work-mode, location and source facets.
- **Offline-first**: the window opens instantly on the cached feed and refreshes
  in the background.

## Project layout

```
app/           the Tauri 2 application (Svelte 5 UI + Rust core)
scripts/       Android SDK/NDK setup
backend/       legacy FastAPI service — superseded, reference only
frontend/      legacy Next.js client — superseded, reference only
```

The `backend/` and `frontend/` directories are the previous two-process version
of this project, kept only as a porting reference. All work happens in `app/`.

## Releases

Pushing a `v*` tag runs `.github/workflows/release.yml`, which builds Linux and
Windows bundles on their native runners and opens a draft release.
