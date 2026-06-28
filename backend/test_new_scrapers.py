import asyncio
import httpx
from scraper_pipeline import TLDRTechScraper, HNTopLinksScraper, DailyDevScraper, HandshakeScraper

async def test_scrapers():
    scrapers = [
        TLDRTechScraper(),
        HNTopLinksScraper(),
        DailyDevScraper(),
        HandshakeScraper()
    ]
    
    async with httpx.AsyncClient(
        timeout=10.0, 
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"}
    ) as client:
        tasks = [scraper.fetch(client) for scraper in scrapers]
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        for scraper, res in zip(scrapers, results):
            if isinstance(res, Exception):
                print(f"[{scraper.source_name}] ERROR: {res}")
            else:
                print(f"[{scraper.source_name}] Fetched {len(res)} items.")
                for item in res[:2]:
                    print(f"  - {item['title']} ({item['url']})")

if __name__ == "__main__":
    asyncio.run(test_scrapers())
