from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from scraper_pipeline import run_pipeline

app = FastAPI()

# Enable CORS for frontend integration
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"], # In production, restrict this to your frontend URL
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.get("/api/hello")
async def hello():
    return {"message": "Hello from FastAPI"}

@app.get("/api/feed")
async def get_feed(major: str = "Software Engineering"):
    """
    Triggers the scraper pipeline and returns the personalized feed.
    """
    feed = await run_pipeline(major)
    return feed
