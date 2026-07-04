import asyncio
from datetime import datetime

from fastapi import Depends, FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, EmailStr
from sqlalchemy.orm import Session

from auth import create_token, get_current_user, hash_password, verify_password
from database import EclassUpdate, User, get_db, init_db, replace_eclass_updates
from eclass_scraper import (
    EclassSessionExpired,
    attempt_state_path,
    auth_attempts,
    auth_popup_login,
    fetch_updates,
    link_account_interactive,
    link_attempts,
    state_path_for,
)
from scraper_pipeline import ItemType, fetch_all_items_cached, get_personalized_feed, run_pipeline

app = FastAPI()
init_db()

# Configure CORS for frontend access
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://localhost:3001"],
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
        "eclass_linked": user.eclass_linked_at is not None,
        "eclass_linked_at": user.eclass_linked_at,
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

# --- YorkU popup sign-in (Passport York + Duo, zero credential handling) ---

@app.post("/api/auth/yorku/start")
async def yorku_auth_start():
    """
    Opens the official YorkU login portal in a browser window on this machine.
    Sign-in (incl. Duo) happens entirely on York's pages; poll
    GET /api/auth/yorku/status/{attempt_id} for the outcome.
    """
    import secrets
    attempt_id = secrets.token_urlsafe(24)
    asyncio.create_task(auth_popup_login(attempt_id, attempt_state_path(attempt_id)))
    return {"attempt_id": attempt_id, "status": "pending"}

@app.get("/api/auth/yorku/status/{attempt_id}")
def yorku_auth_status(attempt_id: str, db: Session = Depends(get_db)):
    attempt = auth_attempts.get(attempt_id)
    if attempt is None:
        raise HTTPException(status_code=404, detail="Unknown sign-in attempt.")
    if attempt["status"] != "success":
        return {"status": attempt["status"], "message": attempt["message"]}

    profile = attempt["profile"]
    user = db.query(User).filter(User.yorku_user_id == profile["moodle_id"]).first()
    if user is None:
        import secrets
        user = User(
            # Synthetic address: YorkU sign-in accounts have no email/password.
            email=f"yorku-{profile['moodle_id']}@antifomo.local",
            password_hash=hash_password(secrets.token_hex(24)),
            name=profile["name"],
            yorku_user_id=profile["moodle_id"],
        )
        db.add(user)
        db.commit()
        db.refresh(user)

    # The captured session becomes the user's eClass session.
    tmp_state = attempt_state_path(attempt_id)
    if tmp_state.exists():
        tmp_state.replace(state_path_for(user.id))
    user.eclass_linked_at = datetime.utcnow()
    db.commit()
    auth_attempts.pop(attempt_id, None)

    return {"status": "success", "token": create_token(user.id), "user": _user_payload(user)}

# --- eClass (YorkU Moodle) integration ---

@app.post("/api/eclass/link/interactive")
async def eclass_link_interactive(user: User = Depends(get_current_user)):
    """
    Opens the official YorkU login portal in a real browser window on this
    machine. The student signs in (and completes Duo) directly on York's
    page; we never see the credentials, only the resulting session. Poll
    GET /api/eclass/link/status for the outcome.
    """
    attempt = link_attempts.get(user.id)
    if attempt and attempt["status"] == "pending":
        return {"status": "pending", "message": "A login window is already open."}
    asyncio.create_task(link_account_interactive(user.id, state_path_for(user.id)))
    return {"status": "pending", "message": "Login window opened — sign in with Passport York."}

@app.get("/api/eclass/link/status")
def eclass_link_status(user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    attempt = link_attempts.get(user.id, {"status": "none", "message": ""})
    if attempt["status"] == "success" and user.eclass_linked_at is None:
        user.eclass_linked_at = datetime.utcnow()
        db.commit()
    return {**attempt, "user": _user_payload(user)}

@app.delete("/api/eclass/link")
def eclass_unlink(user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    state = state_path_for(user.id)
    if state.exists():
        state.unlink()
    db.query(EclassUpdate).filter(EclassUpdate.user_id == user.id).delete()
    user.eclass_linked_at = None
    db.commit()
    link_attempts.pop(user.id, None)
    return {"message": "eClass account unlinked."}

@app.get("/api/eclass/updates")
def eclass_updates(refresh: bool = Query(False), user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """
    Returns the student's eClass courses, upcoming deadlines, and
    announcements. Serves the cached scrape unless refresh=true.
    """
    if user.eclass_linked_at is None:
        raise HTTPException(status_code=409, detail="eClass account not linked yet.")

    cached = db.query(EclassUpdate).filter(EclassUpdate.user_id == user.id).all()
    if refresh or not cached:
        try:
            updates = fetch_updates(state_path_for(user.id))
        except EclassSessionExpired as e:
            raise HTTPException(status_code=401, detail=f"{e} Please re-link your eClass account.")
        replace_eclass_updates(db, user.id, updates)
        cached = db.query(EclassUpdate).filter(EclassUpdate.user_id == user.id).all()

    return {
        "fetched_at": max((u.fetched_at for u in cached), default=None),
        "updates": [
            {
                "kind": u.kind,
                "title": u.title,
                "course": u.course,
                "url": u.url,
                "content_text": u.content_text,
                "timestamp": u.timestamp,
            }
            for u in cached
        ],
    }
