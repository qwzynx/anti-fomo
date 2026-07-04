import asyncio
import httpx
import sys

# Ensure UTF-8 output encoding to support emojis in console
sys.stdout.reconfigure(encoding='utf-8')

from scraper_pipeline import (
    HackerNewsScraper,
    PittCSCGithubScraper,
    PhoronixScraper,
    SimplifyGithubScraper,
    LumaScraper,
    TLDRTechScraper,
    HNTopLinksScraper,
    DailyDevScraper
)

async def check_all_counts():
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
    
    print("Fetching items from all active scrapers... (this may take a few seconds due to Playwright)")
    
    async with httpx.AsyncClient(
        timeout=15.0, 
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"}
    ) as client:
        tasks = [scraper.fetch(client) for scrapers_group in [scrapers] for scraper in scrapers_group]
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        print("\n=== Scraped Items Count by Website/Platform ===")
        total_items = 0
        for scraper, result in zip(scrapers, results):
            if isinstance(result, Exception):
                print(f"❌ {scraper.source_name:<18} : ERROR ({result})")
            else:
                item_count = len(result)
                is_mock = any("[MOCK]" in item.get("title", "") for item in result) if item_count > 0 else False
                mock_label = " (Mock Data)" if is_mock else " (Live Data)"
                print(f"✅ {scraper.source_name:<18} : {item_count:>3} items{mock_label}")
                total_items += item_count
                
        print("===============================================")
        print(f"Total Active Items Scraped: {total_items}")

if __name__ == "__main__":
    asyncio.run(check_all_counts())
