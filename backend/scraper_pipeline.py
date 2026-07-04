import asyncio
import sys

# Set event loop policy on Windows to support subprocesses (needed for Playwright)
if sys.platform == 'win32':
    asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())

import json
import logging
import re
import time
from datetime import datetime
from email.utils import parsedate_to_datetime
from typing import List, Dict, Any, Optional, TypedDict
from enum import Enum

import httpx
from bs4 import BeautifulSoup
import os
from playwright.async_api import async_playwright

# --- Configuration & Types ---

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class ItemType(str, Enum):
    JOB = "Job"
    ARTICLE = "Article"
    EVENT = "Event"
    INTERNSHIP = "Internship"

class ScrapedItem(TypedDict):
    title: str
    source_platform: str
    item_type: ItemType
    url: str
    content_text: str
    timestamp: datetime
    discipline: Optional[str]
    relevance_score: Optional[float]

# Academic Disciplines
MAJORS = {
    "Software Engineering": ["coding", "software", "programming", "python", "java", "api", "web", "cloud", "devops", "intern", "engineer"]
}

# --- Categorization Engine ---

def classify_item(item: ScrapedItem, majors_list: Dict[str, List[str]]) -> str:
    """
    Performs keyword weight analysis to map items to academic disciplines.
    """
    text = f"{item['title']} {item['content_text']}".lower()
    scores = {major: 0 for major in majors_list}

    for major, keywords in majors_list.items():
        for kw in keywords:
            if kw in text:
                scores[major] += 1
    
    # Return highest scoring major or 'General' if no match
    best_match = max(scores, key=scores.get)
    return best_match if scores[best_match] > 0 else "General"

async def classify_with_gemini_ai(text: str) -> str:
    """
    Placeholder for zero-shot classification using google-genai SDK.
    Useful for complex items where keyword matching fails.
    """
    # client = genai.Client(api_key="YOUR_API_KEY")
    # response = client.models.generate_content(model="gemini-2.0-flash", contents=f"Classify this: {text}")
    return "Pending AI Analysis"

# --- Scraper Layer (Modular) ---

class BaseScraper:
    source_name: str = "Base"

    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        raise NotImplementedError

class HackerNewsScraper(BaseScraper):
    source_name = "Hacker News"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            # Using Algolia API for clean JSON
            resp = await client.get("https://hn.algolia.com/api/v1/search_by_date?tags=front_page&hitsPerPage=10")
            data = resp.json()
            for hit in data.get('hits', []):
                items.append({
                    "title": hit.get('title'),
                    "source_platform": self.source_name,
                    "item_type": ItemType.ARTICLE,
                    "url": hit.get('url') or f"https://news.ycombinator.com/item?id={hit.get('objectID')}",
                    "content_text": hit.get('story_text', ""),
                    "timestamp": datetime.fromtimestamp(hit.get('created_at_i', datetime.now().timestamp())),
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class PittCSCGithubScraper(BaseScraper):
    source_name = "Pitt CSC Repo"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://raw.githubusercontent.com/pittcsc/Summer2026-Internships/dev/README.md")
            soup = BeautifulSoup(resp.text, 'html.parser')
            # Newest listings are added at the top; keep the freshest rows
            for row in soup.find_all('tr')[:150]:
                tds = row.find_all('td')
                if len(tds) >= 4:
                    company_a = tds[0].find('a')
                    company = company_a.text.strip() if company_a else tds[0].text.strip()
                    role = tds[1].text.strip()
                    if company == '↳':  # continuation row: same company as above
                        company = items[-1]['title'].removeprefix('Internship at ') if items else 'Unknown'

                    app_link = tds[3].find('a')
                    if app_link and app_link.get('href'):
                        items.append({
                            "title": f"Internship at {company}",
                            "source_platform": self.source_name,
                            "item_type": ItemType.INTERNSHIP,
                            "url": app_link['href'],
                            "content_text": f"Role: {role}",
                            "timestamp": datetime.now(),
                            "discipline": "Software Engineering",
                            "relevance_score": None
                        })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class PhoronixScraper(BaseScraper):
    source_name = "Phoronix"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://www.phoronix.com/rss.php")
            soup = BeautifulSoup(resp.text, 'xml')
            for entry in soup.find_all('item')[:10]:
                items.append({
                    "title": entry.title.text,
                    "source_platform": self.source_name,
                    "item_type": ItemType.ARTICLE,
                    "url": entry.link.text,
                    "content_text": _strip_html(entry.description.text),
                    "timestamp": _rss_date(entry),
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class SimplifyGithubScraper(BaseScraper):
    source_name = "Simplify"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://raw.githubusercontent.com/SimplifyJobs/Summer2026-Internships/dev/README.md")
            soup = BeautifulSoup(resp.text, 'html.parser')
            # Newest listings are added at the top; keep the freshest rows
            for row in soup.find_all('tr')[:150]:
                tds = row.find_all('td')
                if len(tds) >= 4:
                    company_a = tds[0].find('a')
                    company = company_a.text.strip() if company_a else tds[0].text.strip()
                    role = tds[1].text.strip()
                    location = tds[2].text.strip()
                    if company == '↳' and items:  # continuation row: same company as above
                        company = items[-1]['title'].rsplit(' at ', 1)[-1]
                    
                    app_link = tds[3].find('a')
                    if app_link and app_link.get('href'):
                        items.append({
                            "title": f"{role} at {company}",
                            "source_platform": self.source_name,
                            "item_type": ItemType.INTERNSHIP,
                            "url": app_link['href'],
                            "content_text": f"Location: {location}",
                            "timestamp": datetime.now(),
                            "discipline": "Software Engineering",
                            "relevance_score": None
                        })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

def _rss_date(entry) -> datetime:
    """Parse an RSS <pubDate> into a naive datetime, falling back to now()."""
    tag = entry.find('pubDate')
    if tag and tag.text:
        try:
            return parsedate_to_datetime(tag.text.strip()).replace(tzinfo=None)
        except (ValueError, TypeError):
            pass
    return datetime.now()

def _strip_html(text: str) -> str:
    return BeautifulSoup(text or "", 'html.parser').get_text(" ", strip=True)

class LassondeNewsScraper(BaseScraper):
    source_name = "Lassonde News"

    # The site 403s generic clients; a full browser User-Agent (set on the
    # shared httpx client) is required for the WordPress feed to respond.
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            # Note: /news/feed/ is a WordPress *comments* feed; the site-wide
            # /feed/ carries the actual news posts.
            resp = await client.get("https://lassonde.yorku.ca/feed/")
            soup = BeautifulSoup(resp.text, 'xml')
            for entry in soup.find_all('item')[:15]:
                items.append({
                    "title": entry.title.text.strip(),
                    "source_platform": self.source_name,
                    "item_type": ItemType.ARTICLE,
                    "url": entry.link.text.strip(),
                    "content_text": _strip_html(entry.description.text if entry.description else ""),
                    "timestamp": _rss_date(entry),
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class ExperienceYorkScraper(BaseScraper):
    source_name = "Experience York"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        # Gated Content - Providing Mock Data as agreed
        return [
            {
                "title": "[MOCK] Software Developer Intern (Summer 2025)",
                "source_platform": self.source_name,
                "item_type": ItemType.INTERNSHIP,
                "url": "https://experience.yorku.ca/myAccount/career/postings.htm",
                "content_text": "Engineering and Computer Science focus. Log in to Experience York to apply.",
                "timestamp": datetime.now(),
                "discipline": "Software Engineering",
                "relevance_score": None
            }
        ]

class LumaScraper(BaseScraper):
    source_name = "Luma"
    city_page = "https://lu.ma/toronto"

    # lu.ma city pages are Next.js and ship the upcoming-events list inside
    # the __NEXT_DATA__ script tag, so no headless browser is needed.
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get(self.city_page)
            m = re.search(r'<script id="__NEXT_DATA__" type="application/json">(.*?)</script>', resp.text, re.S)
            if not m:
                logger.warning(f"{self.source_name}: __NEXT_DATA__ not found on {self.city_page}")
                return items
            data = json.loads(m.group(1))
            entries = data['props']['pageProps']['initialData']['data'].get('events', [])
            for entry in entries[:20]:
                ev = entry.get('event', {})
                if not ev.get('name'):
                    continue
                geo = ev.get('geo_address_info') or {}
                where = geo.get('city_state') or ("Online" if ev.get('location_type') != 'offline' else "")
                start = ev.get('start_at')
                try:
                    ts = datetime.fromisoformat(start.replace('Z', '+00:00')).replace(tzinfo=None) if start else datetime.now()
                except ValueError:
                    ts = datetime.now()
                items.append({
                    "title": ev['name'],
                    "source_platform": self.source_name,
                    "item_type": ItemType.EVENT,
                    "url": f"https://lu.ma/{ev.get('url', '')}",
                    "content_text": f"{where} — starts {start or 'TBA'}".strip(" —"),
                    "timestamp": ts,
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class LevelsFyiScraper(BaseScraper):
    source_name = "Levels.fyi"

    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://www.levels.fyi/js/internshipData.json")
            data = resp.json()
            current_year = str(datetime.now().year)
            # The dataset is historical; keep only current-year, still-open postings
            fresh = [d for d in data if d.get('yr') == current_year and not d.get('appNotOpen')]
            for d in fresh[:25]:
                salary = f"${d['monthlySalary']}/month" if d.get('monthlySalary') else "Compensation N/A"
                items.append({
                    "title": f"{d.get('title', 'Intern')} at {d.get('company', 'Unknown')}",
                    "source_platform": self.source_name,
                    "item_type": ItemType.INTERNSHIP,
                    "url": d.get('link') or "https://www.levels.fyi/internships/",
                    "content_text": f"{d.get('season', '')} {d.get('yr', '')} · {d.get('loc', 'Location N/A')} · {salary}",
                    "timestamp": datetime.now(),
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class TLDRTechScraper(BaseScraper):
    source_name = "TLDR Tech"

    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            # Find the latest issue from the archives, then scrape its full
            # article blocks (title + summary), skipping sponsor slots.
            archive = await client.get("https://tldr.tech/tech/archives")
            dates = re.findall(r'href="/tech/(\d{4}-\d{2}-\d{2})"', archive.text)
            if not dates:
                logger.warning(f"{self.source_name}: no issues found in archive")
                return items
            latest = max(dates)
            issue = await client.get(f"https://tldr.tech/tech/{latest}")
            soup = BeautifulSoup(issue.text, 'html.parser')
            issue_date = datetime.strptime(latest, "%Y-%m-%d")
            for article in soup.find_all('article'):
                h3 = article.find('h3')
                a_tag = article.find('a', href=True)
                if not h3 or not a_tag:
                    continue
                title = h3.get_text(strip=True)
                if '(sponsor)' in title.lower():
                    continue
                summary_div = article.find('div', class_='newsletter-html')
                items.append({
                    "title": title,
                    "source_platform": self.source_name,
                    "item_type": ItemType.ARTICLE,
                    "url": a_tag['href'],
                    "content_text": summary_div.get_text(" ", strip=True) if summary_div else "",
                    "timestamp": issue_date,
                    "discipline": None,
                    "relevance_score": None
                })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class HNTopLinksScraper(BaseScraper):
    source_name = "HN Top Links"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://hntoplinks.com/")
            soup = BeautifulSoup(resp.text, 'html.parser')
            for story in soup.find_all(class_='story'):
                title_elem = story.find(class_='story-title')
                if title_elem:
                    title = title_elem.text.strip()
                    url = title_elem.get('href', '')
                    items.append({
                        "title": title,
                        "source_platform": self.source_name,
                        "item_type": ItemType.ARTICLE,
                        "url": url,
                        "content_text": "",
                        "timestamp": datetime.now(),
                        "discipline": "Software Engineering",
                        "relevance_score": None
                    })
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class DailyDevScraper(BaseScraper):
    source_name = "Daily.dev"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            async with async_playwright() as p:
                browser = await p.chromium.launch(headless=True)
                page = await browser.new_page()
                await page.goto("https://app.daily.dev/tags/software-engineering", timeout=15000)
                await page.wait_for_timeout(4000)
                
                links = await page.eval_on_selector_all('a', 'elements => elements.map(e => ({text: e.innerText, href: e.href}))')
                for l in links:
                    text = l['text'].strip().replace('\n', ' ')
                    url = l['href']
                    if url and '/posts/' in url and len(text) > 15:
                        items.append({
                            "title": text,
                            "source_platform": self.source_name,
                            "item_type": ItemType.ARTICLE,
                            "url": url,
                            "content_text": "",
                            "timestamp": datetime.now(),
                            "discipline": "Software Engineering",
                            "relevance_score": None
                        })
                        if len(items) >= 15:
                            break
                await browser.close()
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items

class HandshakeScraper(BaseScraper):
    source_name = "Handshake"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        email = os.environ.get("HANDSHAKE_EMAIL")
        password = os.environ.get("HANDSHAKE_PASSWORD")
        
        if not email or not password:
            logger.info(f"{self.source_name}: Credentials not found. Returning mock data.")
            return [
                {
                    "title": "[MOCK] Junior Developer at Local Startup",
                    "source_platform": self.source_name,
                    "item_type": ItemType.JOB,
                    "url": "https://joinhandshake.com/",
                    "content_text": "Requires Handshake credentials in environment variables.",
                    "timestamp": datetime.now(),
                    "discipline": "Software Engineering",
                    "relevance_score": None
                }
            ]
        try:
            async with async_playwright() as p:
                browser = await p.chromium.launch(headless=True)
                page = await browser.new_page()
                await page.goto("https://app.joinhandshake.com/login", timeout=15000)
                await page.fill("input[type='email']", email)
                await page.click("button:has-text('Next')")
                await page.wait_for_timeout(3000)
                items.append({
                    "title": "[MOCK - Partially Auth] Handshake Scraper Execution",
                    "source_platform": self.source_name,
                    "item_type": ItemType.JOB,
                    "url": "https://joinhandshake.com/",
                    "content_text": "SSO handling is complex, placeholder added.",
                    "timestamp": datetime.now(),
                    "discipline": "Software Engineering",
                    "relevance_score": None
                })
                await browser.close()
        except Exception as e:
            logger.error(f"Error scraping {self.source_name}: {e}")
        return items


# --- Prioritization Layer ---

def get_personalized_feed(items: List[ScrapedItem], user_major: str) -> List[ScrapedItem]:
    """
    Calculates relevance scores and ranks items.
    """
    for item in items:
        score = 0.0
        
        # 1. Major Match (Primary Weight)
        if item['discipline'] == user_major:
            score += 10.0
        
        # 2. Item Type Weight
        if item['item_type'] in [ItemType.JOB, ItemType.INTERNSHIP]:
            score += 5.0
        elif item['item_type'] == ItemType.EVENT:
            score += 4.0
        
        # 3. Recency Weight (Placeholder)
        # score += (some_decay_function_of_timestamp)

        item['relevance_score'] = score

    ranked = sorted(items, key=lambda x: x['relevance_score'], reverse=True)

    # Diversify: cap how many items a single source can place up front so one
    # large source (e.g. an internship repo) can't flood the whole feed.
    MAX_PER_SOURCE = 8
    per_source = {}
    head, tail = [], []
    for item in ranked:
        src = item['source_platform']
        per_source[src] = per_source.get(src, 0) + 1
        (head if per_source[src] <= MAX_PER_SOURCE else tail).append(item)
    return head + tail

# --- Main Pipeline ---

def get_scrapers() -> List[BaseScraper]:
    return [
        HackerNewsScraper(),
        PittCSCGithubScraper(),
        PhoronixScraper(),
        SimplifyGithubScraper(),
        LassondeNewsScraper(),
        LumaScraper(),
        LevelsFyiScraper(),
        TLDRTechScraper(),
        HNTopLinksScraper(),
        DailyDevScraper()
    ]

async def fetch_all_items() -> List[ScrapedItem]:
    """
    Runs all scrapers concurrently, dedupes by URL, and classifies disciplines.
    """
    all_items = []

    async with httpx.AsyncClient(
        timeout=20.0,
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"}
    ) as client:
        tasks = [scraper.fetch(client) for scraper in get_scrapers()]
        results = await asyncio.gather(*tasks)

        for result_list in results:
            all_items.extend(result_list)

    seen_urls = set()
    deduped = []
    for item in all_items:
        if item['url'] in seen_urls:
            continue
        seen_urls.add(item['url'])
        if not item['discipline']:
            item['discipline'] = classify_item(item, MAJORS)
        deduped.append(item)

    return deduped

# Scrapes are slow (Playwright); cache results so the home page, internship
# hub, and widgets share one scrape instead of re-fetching per request.
_CACHE_TTL_SECONDS = 600
_cache: Dict[str, Any] = {"items": None, "at": 0.0}

async def fetch_all_items_cached(force: bool = False) -> List[ScrapedItem]:
    if not force and _cache["items"] and time.time() - _cache["at"] < _CACHE_TTL_SECONDS:
        return _cache["items"]
    items = await fetch_all_items()
    if items:
        _cache["items"] = items
        _cache["at"] = time.time()
    return items

async def run_scraper_pipeline_to_db(db):
    """
    Runs all scrapers, classifies their disciplines, and upserts them into the database.
    """
    from database import save_scraped_items
    all_items = await fetch_all_items()
    for item in all_items:
        if item['relevance_score'] is None:
            item['relevance_score'] = 0.0
    save_scraped_items(db, all_items)
    return all_items

async def run_pipeline(target_major: str):
    all_items = await fetch_all_items_cached()
    return get_personalized_feed(all_items, target_major)

if __name__ == "__main__":
    USER_MAJOR = "Software Engineering"
    print(f"--- Generating Prioritized Feed for {USER_MAJOR} ---")
    
    feed = asyncio.run(run_pipeline(USER_MAJOR))
    
    # Output top 15 results as JSON
    print(json.dumps(feed[:15], indent=2, default=str))

