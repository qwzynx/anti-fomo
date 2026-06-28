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
4.  **Keyword Classification**: Let the existing `classify_item` function automatically tag incoming items with disciplines (Software Engineering, Business, etc.).

---

## 3. Implementation Roadmap

- [ ] **Phase 1: RSS & Static Scrapers**
  - [ ] Implement `TLDRTechScraper` using public web archives.
  - [ ] Implement `HNTopLinksScraper` using RSS feeds.
- [ ] **Phase 2: Dynamic API Scrapers**
  - [ ] Implement `LevelsFyiScraper` to extract internship postings.
  - [ ] Implement `DailyDevScraper` via public feeds or API interception.
- [ ] **Phase 3: Authenticated Scrapers**
  - [ ] Implement `HandshakeScraper` using Playwright and secure session-handling.
