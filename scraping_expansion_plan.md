# Scraping Expansion Plan: Anti-FOMO Sources

This document outlines the proposal and plan to integrate additional platforms into the Anti-FOMO aggregator pipeline. These sources will help users consolidate their job search, industry news, and event tracking into a single personalized feed.

---

## 1. Target Platforms Overview

### 🟢 Job & Internship Boards
*   **Levels.fyi (Internships)**
    *   *URL:* [levels.fyi/internships](https://www.levels.fyi/internships/)
    *   *Data Types:* Internship postings, company, compensation/stipend info.
    *   *Scraping Strategy:* Look for internal API requests or use a standard JSON endpoint if available. Fall back to Playwright parsing of the dynamic tables.
*   **Handshake (Student Portal)**
    *   *URL:* [joinhandshake.com](https://joinhandshake.com/)
    *   *Data Types:* Gated student-exclusive job postings, employer events, campus workshops.
    *   *Scraping Strategy:* Requires authentication. We will need session cookie injection and credential storage, similar to the `ExperienceYorkScraper`.

### 🟢 News & Content Aggregators
*   **Daily.dev**
    *   *URL:* [daily.dev](https://daily.dev/)
    *   *Data Types:* Trending developer articles, tutorials, community discussions.
    *   *Scraping Strategy:* Parse their public feeds or target their GraphQL API endpoints if accessible.
*   **TLDR Newsletter**
    *   *URL:* [tldr.tech](https://tldr.tech/)
    *   *Data Types:* Curated daily tech and science article summaries.
    *   *Scraping Strategy:* Parse the web archives of the newsletters or subscribe a dedicated inbox and scrape incoming emails.
*   **Hacker News Digest / Top Links**
    *   *URL:* [hntoplinks.com](https://hntoplinks.com/)
    *   *Data Types:* Summarized high-quality links from Hacker News.
    *   *Scraping Strategy:* Parse their daily/weekly RSS feeds or static archive pages.

---

## 2. Scraping Architecture & Strategy

To support these new scrapers without breaking the pipeline, the following implementation steps are recommended:

1.  **Define New Scraper Classes**: Extend `BaseScraper` in `backend/scraper_pipeline.py` for each source.
2.  **Handle Authentication/JS**:
    *   Use `playwright` for gated sites (Handshake).
    *   Use `httpx` and `BeautifulSoup` for simple RSS/Markdown/HTML sources (TLDR, HN Digest).
3.  **Data Standardizing**: Map incoming items to the existing `ScrapedItem` schema:
    *   `title`
    *   `source_platform`
    *   `item_type` (Job, Article, Event, Internship)
    *   `url`
    *   `content_text`
    *   `timestamp`
4.  **Keyword Classification**: Let the existing `classify_item` function automatically tag incoming items with disciplines (Software Engineering, Data Science, etc.).

---

## 3. Implementation Roadmap

- [x] **Phase 1: RSS & Static Scrapers**
  - [x] Implement `TLDRTechScraper` — scrapes the latest issue from the archives with full article summaries (sponsor slots filtered).
  - [x] Implement `HNTopLinksScraper` using the site's story listing.
  - [x] Implement `LassondeNewsScraper` — real WordPress RSS at `https://lassonde.yorku.ca/feed/` (requires a browser User-Agent; `/news/feed/` is a comments feed, do not use it).
- [x] **Phase 2: Dynamic API Scrapers**
  - [x] Implement `LevelsFyiScraper` — public dataset at `https://www.levels.fyi/js/internshipData.json`, filtered to current-year open postings.
  - [x] Implement `DailyDevScraper` via Playwright.
  - [x] Implement `LumaScraper` — parses the events embedded in `__NEXT_DATA__` on lu.ma city pages (currently Toronto).
- [ ] **Phase 3: Authenticated Scrapers**
  - [x] Implement eClass (YorkU Moodle) scraper — see below.
  - [ ] Implement `HandshakeScraper` using Playwright and secure session-handling.
  - [ ] Implement Experience York scraper (Passport York SSO; can reuse the eClass session approach).

---

## 4. Student Accounts & eClass Integration (implemented)

- **Accounts:** students register/login (`/api/auth/register`, `/api/auth/login`, JWT bearer auth). User data lives in the `users` table (`backend/database.py`); local dev defaults to SQLite (`antifomo.db`), Postgres via `DATABASE_URL`.
- **eClass link:** `POST /api/eclass/link` drives a Playwright login through YorkU's Shibboleth SSO (`shib.yorku.ca` / Passport York) including Duo push approval. Passport York credentials are used transiently and never stored — only the Playwright session state, saved per user under `backend/data/eclass_sessions/`.
- **Updates:** `GET /api/eclass/updates` uses the saved session against Moodle's AJAX API (`lib/ajax/service.php` with the page `sesskey`) to pull in-progress courses, upcoming deadlines (`core_calendar_get_action_events_by_timesort`), and notifications/announcements (`message_popup_get_popup_notifications`). Results are cached in the `eclass_updates` table; pass `refresh=true` to re-scrape. When the session expires the endpoint returns 401 and the student re-links.
- **Frontend:** `/login` (register + sign-in) and `/dashboard` (Student Hub: deadlines, announcements, courses, link/unlink flow) in `frontend/src/app/`.
