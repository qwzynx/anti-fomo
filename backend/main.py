from fastapi import FastAPI, Query
from fastapi.middleware.cors import CORSMiddleware
from scraper_pipeline import run_pipeline

app = FastAPI()

# Configure CORS for frontend access
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

@app.get("/api/hello")
async def hello():
    return {"message": "Hello from FastAPI"}

@app.get("/api/feed")
async def get_feed(major: str = Query("Software Engineering")):
    """
    Returns a prioritized feed of articles, jobs, and events.
    """
    feed = await run_pipeline(major)
    return feed[:30] # Return top 30 items
