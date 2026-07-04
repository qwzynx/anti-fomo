import asyncio
import sys

# Set event loop policy on Windows to support subprocesses (needed for Playwright)
if sys.platform == 'win32':
    asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())

import json
import logging
from datetime import datetime
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
    "Software Engineering": ["coding", "software", "programming", "python", "java", "api", "web", "cloud", "devops", "intern", "engineer"],
    "Mechanical Engineering": ["robotics", "cad", "thermodynamics", "manufacturing", "automotive", "mechanics"],
    "Civil Engineering": ["structural", "construction", "transportation", "urban", "infrastructure", "surveying"],
    "Business": ["marketing", "finance", "startup", "management", "consulting", "economics"]
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
            for row in soup.find_all('tr'):
                tds = row.find_all('td')
                if len(tds) >= 4:
                    company_a = tds[0].find('a')
                    company = company_a.text.strip() if company_a else tds[0].text.strip()
                    role = tds[1].text.strip()
                    
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
                    "content_text": entry.description.text,
                    "timestamp": datetime.now(), # RSS usually has pubDate, but using now for brevity
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
            for row in soup.find_all('tr'):
                tds = row.find_all('td')
                if len(tds) >= 4:
                    company_a = tds[0].find('a')
                    company = company_a.text.strip() if company_a else tds[0].text.strip()
                    role = tds[1].text.strip()
                    location = tds[2].text.strip()
                    
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

class LassondeNewsScraper(BaseScraper):
    source_name = "Lassonde News"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        # Persistent 403 Forbidden - Providing Mock Data to ensure a functional demo
        return [
            {
                "title": "[MOCK] Lassonde Researchers Develop New AI for Climate Prediction",
                "source_platform": self.source_name,
                "item_type": ItemType.ARTICLE,
                "url": "https://lassonde.yorku.ca/news/",
                "content_text": "A breakthrough study by Lassonde School of Engineering professors has led to a more accurate model for predicting local weather patterns.",
                "timestamp": datetime.now(),
                "discipline": "Software Engineering",
                "relevance_score": None
            },
            {
                "title": "[MOCK] Engineering Student Team Wins International Robotics Competition",
                "source_platform": self.source_name,
                "item_type": ItemType.ARTICLE,
                "url": "https://lassonde.yorku.ca/news/",
                "content_text": "The Lassonde Robotics team took first place in the global challenge held in Berlin last week.",
                "timestamp": datetime.now(),
                "discipline": "Mechanical Engineering",
                "relevance_score": None
            }
        ]

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
            },
            {
                "title": "[MOCK] Business Analyst Intern",
                "source_platform": self.source_name,
                "item_type": ItemType.INTERNSHIP,
                "url": "https://experience.yorku.ca/myAccount/career/postings.htm",
                "content_text": "Lassonde Professional Internship Program listing.",
                "timestamp": datetime.now(),
                "discipline": "Business",
                "relevance_score": None
            }
        ]

class LumaScraper(BaseScraper):
    source_name = "Luma"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        # Gated/Community specific - Providing Mock Data
        return [
            {
                "title": "[MOCK] YorkU Tech Mixer",
                "source_platform": self.source_name,
                "item_type": ItemType.EVENT,
                "url": "https://lu.ma/yorku-tech",
                "content_text": "Networking event for Lassonde and Schulich students.",
                "timestamp": datetime.now(),
                "discipline": "General",
                "relevance_score": None
            }
        ]

# Note: Experience York, Luma, and Simplify would ideally use Playwright for dynamic content.
# This template demonstrates the BS4/Requests flow which works for their public feeds/SEO pages.

class TLDRTechScraper(BaseScraper):
    source_name = "TLDR Tech"
    
    async def fetch(self, client: httpx.AsyncClient) -> List[ScrapedItem]:
        items = []
        try:
            resp = await client.get("https://tldr.tech/tech")
            soup = BeautifulSoup(resp.text, 'html.parser')
            for h3 in soup.find_all('h3'):
                a_tag = h3.find_parent('a')
                if not a_tag:
                    a_tag = h3.find('a') if h3.name == 'h3' else None
                if not a_tag and h3.parent.name == 'a':
                    a_tag = h3.parent
                if a_tag:
                    title = h3.text.strip()
                    url = a_tag.get('href', '')
                    if url.startswith('/'):
                        url = f"https://tldr.tech{url}"
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
            items.append({
                "title": "[MOCK] Top 10 React Libraries in 2026",
                "source_platform": self.source_name,
                "item_type": ItemType.ARTICLE,
                "url": "https://daily.dev",
                "content_text": "Mock data due to fetch error.",
                "timestamp": datetime.now(),
                "discipline": "Software Engineering",
                "relevance_score": None
            })
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
        
        # 3. Recency Weight (Placeholder)
        # score += (some_decay_function_of_timestamp)

        item['relevance_score'] = score

    # Sort by score descending
    return sorted(items, key=lambda x: x['relevance_score'], reverse=True)

# --- Main Pipeline ---

async def run_scraper_pipeline_to_db(db):
    """
    Runs all scrapers, classifies their disciplines, and upserts them into the database.
    """
    from database import save_scraped_items
    scrapers = [
        HackerNewsScraper(),
        PittCSCGithubScraper(),
        PhoronixScraper(),
        SimplifyGithubScraper(),
        LumaScraper(),
        TLDRTechScraper(),
        HNTopLinksScraper(),
        DailyDevScraper()
    ]
    
    all_items = []
    
    async with httpx.AsyncClient(
        timeout=10.0, 
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"}
    ) as client:
        # Concurrent fetching
        tasks = [scraper.fetch(client) for scraper in scrapers]
        results = await asyncio.gather(*tasks)
        
        for result_list in results:
            all_items.extend(result_list)
            
    # Process items
    for item in all_items:
        if not item['discipline']:
            item['discipline'] = classify_item(item, MAJORS)
        if item['relevance_score'] is None:
            item['relevance_score'] = 0.0
            
    # Save/Upsert to database
    save_scraped_items(db, all_items)
    return all_items

async def run_pipeline(target_major: str):
    scrapers = [
        HackerNewsScraper(),
        PittCSCGithubScraper(),
        PhoronixScraper(),
        SimplifyGithubScraper(),
        LumaScraper(),
        TLDRTechScraper(),
        HNTopLinksScraper(),
        DailyDevScraper()
    ]
    
    all_items = []
    
    async with httpx.AsyncClient(
        timeout=10.0, 
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"}
    ) as client:
        # Concurrent fetching
        tasks = [scraper.fetch(client) for scraper in scrapers]
        results = await asyncio.gather(*tasks)
        
        for result_list in results:
            all_items.extend(result_list)
            
    # Process items
    for item in all_items:
        if not item['discipline']:
            item['discipline'] = classify_item(item, MAJORS)
            
    # Rank items
    personalized_feed = get_personalized_feed(all_items, target_major)
    
    return personalized_feed

if __name__ == "__main__":
    USER_MAJOR = "Software Engineering"
    print(f"--- Generating Prioritized Feed for {USER_MAJOR} ---")
    
    feed = asyncio.run(run_pipeline(USER_MAJOR))
    
    # Output top 15 results as JSON
    print(json.dumps(feed[:15], indent=2, default=str))

