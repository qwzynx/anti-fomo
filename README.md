# Anti-FOMO

<div align="center">

![Next.js](https://img.shields.io/badge/Next.js_16-black?style=for-the-badge&logo=next.js&logoColor=white)
![React](https://img.shields.io/badge/React_19-20232A?style=for-the-badge&logo=react&logoColor=61DAFB)
![TypeScript](https://img.shields.io/badge/TypeScript-007ACC?style=for-the-badge&logo=typescript&logoColor=white)
![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS_4-38B2AC?style=for-the-badge&logo=tailwind-css&logoColor=white)
![FastAPI](https://img.shields.io/badge/FastAPI-009688?style=for-the-badge&logo=fastapi&logoColor=white)
![Python](https://img.shields.io/badge/Python_3.10+-3776AB?style=for-the-badge&logo=python&logoColor=white)
![Playwright](https://img.shields.io/badge/Playwright-2EAD33?style=for-the-badge&logo=playwright&logoColor=white)
![SQLAlchemy](https://img.shields.io/badge/SQLAlchemy-D71F00?style=for-the-badge&logo=python&logoColor=white)

**A personal aggregator that pulls internships, tech news, and events into one ranked feed — with an optional YorkU eClass hub for students there.**

[Features](#features) • [Tech Stack](#tech-stack) • [Getting Started](#getting-started) • [API Overview](#api-overview) • [Project Structure](#project-structure)

</div>

---

## About

Job postings live on GitHub internship-tracker repos, tech news lives in newsletters and link aggregators, and (for York University students) course deadlines live behind a Moodle login — three different places to check every day. Anti-FOMO scrapes all of them concurrently, dedupes and tags the results, and serves a single ranked feed through a Next.js frontend.

It has two halves:

- **A FastAPI backend** that runs an async scraper pipeline against ten public sources, normalizes locations, scores relevance per user, and stores everything in SQLite (or Postgres) via SQLAlchemy.
- **A Next.js frontend** that renders the feed with filtering/sorting, a dedicated internships hub, JWT-based accounts, and (for YorkU students) a dashboard that links an eClass account through a real browser popup to support Duo 2FA natively.

## Features

### Aggregated feed
- Ten scrapers run concurrently (`httpx` + `BeautifulSoup4` for static sources, Playwright for JS-rendered ones), covering GitHub internship-tracker repos, Hacker News, RSS feeds, and event listings. Results are deduplicated by URL and cached for 10 minutes so repeat requests don't re-scrape.
- Each item is auto-classified into an academic discipline via keyword matching and given a relevance score based on discipline match, item type, and a per-source cap so one large source can't flood the feed.
- The home feed (`/`) supports filtering by item type (Internship/Event/Article), source platform, freshness (last 24h / week / month), and sorting by relevance or recency.

### Internships hub (`/internships`)
- All scraped jobs/internships in one view, independent of the 60-item home feed cap.
- A location-normalization step maps raw location strings (from markdown tables, RSS text, etc.) to structured tags: modality (Remote / Hybrid / On-site), region (Canada / USA / Global-Multi-region), and hub cities (Toronto, Vancouver, Waterloo, San Francisco, New York, Seattle, London).
- Multi-select filters for modality, region/city, source, and discipline, plus free-text search across title, company, and description.

### Accounts
- Email/password registration and login, with PBKDF2-hashed passwords and JWT bearer tokens (`auth.py`).
- A separate passwordless path: **Sign in with YorkU** opens the official Passport York SSO login in a real browser window (Playwright, headed); once signed in, the app resolves the student's Moodle identity and creates or logs into a matching Anti-FOMO account. No password is ever entered into Anti-FOMO's own forms for this path.

### YorkU eClass dashboard (`/dashboard`)
- Students can link their eClass account through the same popup flow used for sign-in (`POST /api/eclass/link/interactive`). Because the login happens on York's own domain, Duo 2FA (push, OTP, security keys) works exactly as it does on the real site.
- Only the resulting Playwright session cookies are persisted to disk per user (`backend/data/eclass_sessions/`) — Anti-FOMO never sees or stores a York password.
- Once linked, the backend calls Moodle's session-authenticated AJAX API directly (no DOM scraping) to pull enrolled courses, upcoming deadlines/calendar events, and notifications, and caches them in the database until the student refreshes or unlinks.

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend framework | [Next.js 16](https://nextjs.org/) (App Router), [React 19](https://react.dev/) |
| Language | [TypeScript](https://www.typescriptlang.org/) |
| Styling | [Tailwind CSS 4](https://tailwindcss.com/) (CSS-first config via `@theme`) |
| Backend framework | [FastAPI](https://fastapi.tiangolo.com/) on [Uvicorn](https://www.uvicorn.org/) |
| Scraping | [httpx](https://www.python-httpx.org/) (async HTTP), [BeautifulSoup4](https://www.crummy.com/software/BeautifulSoup/) + `lxml` (HTML/RSS parsing), [Playwright for Python](https://playwright.dev/python/) (JS-rendered pages, YorkU login capture) |
| Database / ORM | [SQLAlchemy](https://www.sqlalchemy.org/) — SQLite by default, PostgreSQL via `DATABASE_URL` |
| Auth | [PyJWT](https://pyjwt.readthedocs.io/) bearer tokens, PBKDF2-HMAC password hashing |

## Getting Started

### Prerequisites
- Node.js 20+
- Python 3.10+
- Git

### Clone

```bash
git clone https://github.com/qwzynx/anti-fomo.git
cd anti-fomo
```

### Backend

```bash
cd backend
python -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate

pip install -r requirements.txt
playwright install chromium   # required for the eClass/YorkU login popup and Daily.dev scraper

uvicorn main:app --reload --port 8000
```

The API is now served at `http://localhost:8000`, with interactive Swagger docs at `http://localhost:8000/docs`. On first run it creates a local `antifomo.db` SQLite file automatically — no database setup needed for local development.

### Frontend

In a second terminal:

```bash
cd frontend
npm install
npm run dev
```

The app is now served at `http://localhost:3000` and talks to the backend at `http://localhost:8000` by default.

### Environment variables

Neither app ships a `.env.example`; both fall back to sensible local defaults, so nothing is required to run locally. To override defaults, create a `.env` file in the relevant directory (both are git-ignored):

**`backend/.env`**

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | `sqlite:///./antifomo.db` | SQLAlchemy connection string. Point this at Postgres in production (`postgres://…` is auto-normalized to `postgresql://…`). |
| `SECRET_KEY` | `dev-secret-change-me` | Signing key for JWT auth tokens. Set a real secret outside local dev. |

**`frontend/.env.local`**

| Variable | Default | Purpose |
|---|---|---|
| `NEXT_PUBLIC_API_BASE` | `http://localhost:8000` | Base URL the frontend uses to reach the FastAPI backend. |

### Verifying the scrapers

`check_scraper_counts.py` is a CLI diagnostic that runs a subset of the scrapers directly and prints how many live items each one returns — useful for spotting a source that's started returning zero items (usually a sign the target site changed its markup):

```bash
cd backend
source .venv/bin/activate
python check_scraper_counts.py
```

## API Overview

All routes are defined in `backend/main.py`.

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/feed` | Top 60 items from the cached scrape, ranked for a given `major` query param. |
| `GET` | `/api/internships` | Every scraped job/internship, ranked (used by the Internships hub; client filters further). |
| `POST` | `/api/auth/register` | Create an account with email + password. |
| `POST` | `/api/auth/login` | Log in, returns a JWT. |
| `GET` | `/api/auth/me` | Current user from the bearer token. |
| `POST` | `/api/auth/yorku/start` | Opens the Passport York login popup as a sign-in method; returns an `attempt_id` to poll. |
| `GET` | `/api/auth/yorku/status/{attempt_id}` | Poll the outcome of a YorkU sign-in attempt. |
| `POST` | `/api/eclass/link/interactive` | Opens the same popup to link eClass to an already-logged-in account. |
| `GET` | `/api/eclass/link/status` | Poll the outcome of an eClass linking attempt. |
| `DELETE` | `/api/eclass/link` | Unlink eClass and delete cached updates for the current user. |
| `GET` | `/api/eclass/updates` | Cached (or freshly scraped, with `?refresh=true`) courses/deadlines/notifications. |

## Project Structure

```text
anti-fomo/
├── backend/
│   ├── main.py                    # FastAPI app: CORS, feed/internship routes, auth, eClass routes
│   ├── scraper_pipeline.py        # BaseScraper subclasses per source, location normalization, relevance scoring
│   ├── eclass_scraper.py          # Playwright popup login capture + Moodle AJAX client for eClass data
│   ├── database.py                # SQLAlchemy models (DBScrapedItem, User, EclassUpdate) and upsert helpers
│   ├── auth.py                    # Password hashing (PBKDF2) and JWT issuance/verification
│   ├── check_scraper_counts.py    # CLI script to sanity-check scraper output counts
│   ├── data/eclass_sessions/      # Per-user Playwright session state (git-ignored)
│   └── requirements.txt
├── frontend/
│   ├── src/
│   │   ├── app/
│   │   │   ├── page.tsx           # Home feed: filters (type/source/freshness), sort, search
│   │   │   ├── internships/       # Internships hub: location/modality/region filters
│   │   │   ├── dashboard/         # eClass linking flow + courses/deadlines/announcements view
│   │   │   └── login/             # Email/password + "Sign in with YorkU" popup
│   │   ├── components/            # Header, ItemCard, ItemModal
│   │   └── lib/api.ts             # Typed fetch wrapper, token storage, shared types
│   └── package.json
├── internship_filters_and_yorku_signin_plan.md   # Design notes for location filters & YorkU popup sign-in
├── frontend_enhancement_plan.md                  # Design notes for the UI/UX pass
└── scraping_expansion_plan.md                    # Design notes for adding new scraper sources
```

## Notes on scope

A couple of things worth knowing if you're reading the code alongside this README:

- `scraper_pipeline.py` defines `ExperienceYorkScraper` and `HandshakeScraper` classes, but neither is wired into the active pipeline (`get_scrapers()`) — they exist as scaffolding for gated sources that need real credentials, and currently return mock/placeholder data if invoked directly.
- There's a `classify_with_gemini_ai` stub in the same file; it isn't called anywhere yet. Discipline classification in the running app is keyword-based (`classify_item`).
