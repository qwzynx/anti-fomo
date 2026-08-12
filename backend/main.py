from fastapi import Depends, FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, EmailStr
from sqlalchemy.orm import Session

from auth import create_token, get_current_user, hash_password, verify_password
from database import User, get_db, init_db
from scraper_pipeline import ItemType, fetch_all_items_cached, get_personalized_feed, run_pipeline

app = FastAPI()
init_db()

# Configure CORS for frontend access
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:3000",
        "http://localhost:3001",
        "http://192.168.2.12:3000",
        "http://192.168.2.12:3001",
    ],
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
    return feed[:60]  # Return top 60 items

@app.get("/api/internships")
async def get_internships(major: str = Query("Software Engineering")):
    """
    Returns every scraped job/internship (no per-source cap) ranked by
    relevance, for the dedicated internship hub. Filtering happens client-side.
    """
    items = await fetch_all_items_cached()
    jobs = [i for i in items if i['item_type'] in (ItemType.JOB, ItemType.INTERNSHIP)]
    for job in jobs:
        job['relevance_score'] = (10.0 if job['discipline'] == major else 0.0) + 5.0
    return sorted(jobs, key=lambda x: x['relevance_score'], reverse=True)

# --- Student accounts ---

class RegisterRequest(BaseModel):
    email: EmailStr
    password: str
    name: str = ""
    major: str = "Software Engineering"

class LoginRequest(BaseModel):
    email: EmailStr
    password: str

def _user_payload(user: User) -> dict:
    return {
        "id": user.id,
        "email": user.email,
        "name": user.name,
        "major": user.major,
    }

@app.post("/api/auth/register")
def register(req: RegisterRequest, db: Session = Depends(get_db)):
    if len(req.password) < 8:
        raise HTTPException(status_code=400, detail="Password must be at least 8 characters.")
    if db.query(User).filter(User.email == req.email).first():
        raise HTTPException(status_code=409, detail="An account with this email already exists.")
    user = User(email=req.email, password_hash=hash_password(req.password), name=req.name, major=req.major)
    db.add(user)
    db.commit()
    db.refresh(user)
    return {"token": create_token(user.id), "user": _user_payload(user)}

@app.post("/api/auth/login")
def login(req: LoginRequest, db: Session = Depends(get_db)):
    user = db.query(User).filter(User.email == req.email).first()
    if not user or not verify_password(req.password, user.password_hash):
        raise HTTPException(status_code=401, detail="Invalid email or password.")
    return {"token": create_token(user.id), "user": _user_payload(user)}

@app.get("/api/auth/me")
def me(user: User = Depends(get_current_user)):
    return _user_payload(user)
