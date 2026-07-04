# Anti-FOMO Project Guidelines for Claude

## Overview
**Anti-FOMO** is a personalized aggregator pipeline designed to consolidate job search (internships, jobs), industry news, and event tracking into a single unified feed. The goal is to provide a unified portal that helps users stay up to date without experiencing "Fear Of Missing Out" (FOMO) across different platforms.

## Architecture & Tech Stack

### Backend (`/backend`)
- **Framework:** Python with FastAPI (`main.py`).
- **Functionality:** Exposes an `/api/feed` endpoint that retrieves a prioritized feed of articles, jobs, and events.
- **Scraping Pipeline:** Data is gathered via a custom scraping pipeline (`scraper_pipeline.py`). It consists of scrapers (inheriting from `BaseScraper`) to extract items (Jobs, Articles, Events) from various platforms (e.g., Levels.fyi, Handshake, Daily.dev, TLDR, Hacker News). Items are automatically classified by discipline (e.g., Software Engineering).
- **Documentation:** For information on adding new scraping sources, refer to `scraping_expansion_plan.md`.

### Frontend (`/frontend`)
- **Framework:** Next.js (TypeScript).
- **Important Note:** Read `frontend/AGENTS.md` for Next.js agent rules. This project uses newer Next.js patterns, which may contain breaking changes compared to your training data. Always check documentation inside `node_modules/next/dist/docs/` if you are unsure about API conventions.

## Current Goals
1. Enhance the scraping pipeline to pull from more diverse platforms (static RSS/Web and dynamic API/Playwright based).
2. Standardize all incoming scraped items to a common `ScrapedItem` schema.
3. Build a beautiful, responsive, and highly engaging UI in Next.js to consume the backend API and render the customized feed.

## Agent Guidelines
- When adding new scrapers, follow the patterns outlined in `scraping_expansion_plan.md`.
- Be aware of the `CLAUDE.md` and `AGENTS.md` directives in the `/frontend` directory when making modifications to the UI.
- Preserve existing documentation and code structures unless explicitly instructed otherwise.
