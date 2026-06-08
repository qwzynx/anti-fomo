import asyncio
import json
import logging
from datetime import datetime
from typing import List, Dict, Any, Optional, TypedDict
from enum import Enum

import httpx
from bs4 import BeautifulSoup

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
            # Simplified: Scraping the Readme or a specific JSON if available
            # In a real scenario, use the GitHub API for reliability
            resp = await client.get("https://raw.githubusercontent.com/pittcsc/Summer2025-Internships/dev/README.md")
            lines = resp.text.split('\n')
            for line in lines:
                if "|" in line and "http" in line:
                    # Basic parser for Markdown table rows
                    parts = [p.strip() for p in line.split('|')]
                    if len(parts) > 3 and "http" in parts[3]:
                        items.append({
                            "title": f"Internship at {parts[1]}",
                            "source_platform": self.source_name,
                            "item_type": ItemType.INTERNSHIP,
                            "url": parts[3].split('(')[1].split(')')[0] if '(' in parts[3] else parts[3],
                            "content_text": f"Role: {parts[2]}",
                            "timestamp": datetime.now(),
                            "discipline": "Software Engineering", # Predetermined for this source
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
            resp = await client.get("https://www.phoronix.com/phoronix.rss")
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

# Note: Experience York, Luma, and Simplify would ideally use Playwright for dynamic content.
# This template demonstrates the BS4/Requests flow which works for their public feeds/SEO pages.

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

async def run_pipeline(target_major: str):
    scrapers = [
        HackerNewsScraper(),
        PittCSCGithubScraper(),
        PhoronixScraper()
    ]
    
    all_items = []
    
    async with httpx.AsyncClient(timeout=10.0, follow_redirects=True) as client:
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
