"""
Scraper for York University's eClass (Moodle) portal.

Login goes through YorkU's Shibboleth SSO (shib.yorku.ca) with Duo 2FA, so
every sign-in happens in a real (headed) browser window showing the official
Passport York portal — Anti-FOMO never sees or handles credentials. Only the
resulting session cookies (Playwright storage state) are persisted, per user.
Data is then pulled through Moodle's session-authenticated AJAX API with
plain httpx, which is far faster and more stable than DOM scraping.
"""

import asyncio
import json
import os
import re
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

import httpx
from playwright.async_api import async_playwright

ECLASS_BASE = "https://eclass.yorku.ca"
LOGIN_URL = f"{ECLASS_BASE}/login/index.php"
DASHBOARD_URL = f"{ECLASS_BASE}/my/"

SESSIONS_DIR = Path(__file__).parent / "data" / "eclass_sessions"


class EclassSessionExpired(Exception):
    """Saved eClass session no longer works; the student must re-link."""


# Status of in-flight popup attempts. `link_attempts` is keyed by user id
# (linking eClass from the dashboard); `auth_attempts` by a random attempt id
# (signing in to Anti-FOMO itself). Single-process app, so dicts suffice.
link_attempts: Dict[int, Dict[str, str]] = {}
auth_attempts: Dict[str, Dict[str, Any]] = {}


def state_path_for(user_id: int) -> Path:
    SESSIONS_DIR.mkdir(parents=True, exist_ok=True)
    return SESSIONS_DIR / f"user_{user_id}.json"


def attempt_state_path(attempt_id: str) -> Path:
    SESSIONS_DIR.mkdir(parents=True, exist_ok=True)
    return SESSIONS_DIR / f"attempt_{attempt_id}.json"


async def _popup_login(state_path: Path) -> Dict[str, Any]:
    """
    Opens a real (headed) browser window on the official YorkU login portal
    and waits for the student to finish signing in there (incl. Duo 2FA).
    Saves the session state on success. Returns {"success", "message"}.
    """
    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=False, args=["--window-size=520,760"])
            context = await browser.new_context(viewport=None)
            page = await context.new_page()
            await page.goto(LOGIN_URL, timeout=30000)

            deadline = time.monotonic() + 300  # give the student 5 minutes
            while time.monotonic() < deadline:
                try:
                    if page.is_closed():
                        return {"success": False, "message": "The login window was closed before finishing."}
                    url = page.url
                    if url.startswith(ECLASS_BASE) and "/login" not in url:
                        await context.storage_state(path=str(state_path))
                        os.chmod(state_path, 0o600)
                        await browser.close()
                        return {"success": True, "message": "Signed in to YorkU."}
                    await page.wait_for_timeout(1000)
                except Exception:
                    return {"success": False, "message": "The login window was closed before finishing."}

            await browser.close()
            return {"success": False, "message": "Timed out waiting for the YorkU login to finish."}
    except Exception as e:
        return {"success": False, "message": f"Could not open the login window: {e}"}


async def link_account_interactive(user_id: int, state_path: Path) -> None:
    """Popup link flow for an already signed-in student (dashboard)."""
    link_attempts[user_id] = {"status": "pending", "message": "Waiting for you to sign in to YorkU…"}
    result = await _popup_login(state_path)
    link_attempts[user_id] = {
        "status": "success" if result["success"] else "failed",
        "message": "eClass account linked." if result["success"] else result["message"],
    }


async def auth_popup_login(attempt_id: str, state_path: Path) -> None:
    """
    Popup flow used as the app's sign-in: after the YorkU login completes,
    resolve the student's Moodle identity so the API can find or create
    their Anti-FOMO account.
    """
    auth_attempts[attempt_id] = {"status": "pending", "message": "Waiting for you to sign in to YorkU…"}
    result = await _popup_login(state_path)
    if not result["success"]:
        auth_attempts[attempt_id] = {"status": "failed", "message": result["message"]}
        return
    profile = await asyncio.to_thread(fetch_profile, state_path)
    if not profile:
        auth_attempts[attempt_id] = {"status": "failed", "message": "Signed in, but could not read your eClass profile."}
        return
    auth_attempts[attempt_id] = {"status": "success", "message": "Signed in to YorkU.", "profile": profile}


def fetch_profile(state_path: Path) -> Optional[Dict[str, Any]]:
    """
    Resolves the logged-in student's Moodle user id and display name from
    their own profile page. Returns {"moodle_id": int, "name": str} or None.
    """
    try:
        with _client_from_state(state_path) as client:
            resp = client.get(f"{ECLASS_BASE}/user/profile.php")
            text = resp.text
            moodle_id = None
            for pattern in (r'[?&]id=(\d+)',):
                m = re.search(pattern, str(resp.url))
                if m:
                    moodle_id = int(m.group(1))
                    break
            if moodle_id is None:
                for pattern in (r'user/profile\.php\?id=(\d+)', r'"userid"\s*:\s*(\d+)', r'data-userid="(\d+)"'):
                    m = re.search(pattern, text)
                    if m:
                        moodle_id = int(m.group(1))
                        break
            if moodle_id is None:
                return None

            name = None
            m = re.search(r'<h1[^>]*>\s*([^<]+?)\s*</h1>', text)
            if m:
                name = m.group(1).strip()
            if not name:
                m = re.search(r'<title>\s*([^<:|]+)', text)
                name = m.group(1).strip() if m else "YorkU Student"
            return {"moodle_id": moodle_id, "name": name}
    except Exception:
        return None


def _client_from_state(state_path: Path) -> httpx.Client:
    state = json.loads(state_path.read_text())
    client = httpx.Client(
        timeout=20.0,
        follow_redirects=True,
        headers={"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"},
    )
    for cookie in state.get("cookies", []):
        if "yorku.ca" in cookie.get("domain", ""):
            client.cookies.set(cookie["name"], cookie["value"], domain=cookie["domain"], path=cookie.get("path", "/"))
    return client


def _ajax(client: httpx.Client, sesskey: str, methodname: str, args: Dict[str, Any]) -> Optional[Any]:
    resp = client.post(
        f"{ECLASS_BASE}/lib/ajax/service.php",
        params={"sesskey": sesskey, "info": methodname},
        json=[{"index": 0, "methodname": methodname, "args": args}],
    )
    try:
        payload = resp.json()[0]
    except (ValueError, IndexError, KeyError):
        return None
    if payload.get("error"):
        return None
    return payload.get("data")


def fetch_updates(state_path: Path) -> List[Dict[str, Any]]:
    """
    Uses the saved session to pull courses, upcoming deadlines/events, and
    notifications (incl. course announcements). Raises EclassSessionExpired
    if the session no longer authenticates.
    """
    if not state_path.exists():
        raise EclassSessionExpired("No saved eClass session.")

    with _client_from_state(state_path) as client:
        dash = client.get(DASHBOARD_URL)
        if "/login" in str(dash.url) or "shib.yorku.ca" in str(dash.url):
            raise EclassSessionExpired("eClass session expired.")
        m = re.search(r'"sesskey":"(\w+)"', dash.text)
        if not m:
            raise EclassSessionExpired("Could not establish a Moodle session.")
        sesskey = m.group(1)

        updates: List[Dict[str, Any]] = []

        courses = _ajax(client, sesskey, "core_course_get_enrolled_courses_by_timeline_classification",
                        {"classification": "inprogress", "limit": 0, "offset": 0, "sort": "fullname"})
        course_names: Dict[int, str] = {}
        for c in (courses or {}).get("courses", []):
            course_names[c["id"]] = c.get("fullname", "")
            updates.append({
                "kind": "course",
                "title": c.get("fullname", "Course"),
                "course": c.get("shortname"),
                "url": f"{ECLASS_BASE}/course/view.php?id={c['id']}",
                "content_text": re.sub(r"<[^>]+>", " ", c.get("summary") or "").strip()[:500],
                "timestamp": None,
            })

        events = _ajax(client, sesskey, "core_calendar_get_action_events_by_timesort",
                       {"limitnum": 26, "timesortfrom": int(time.time()) - 86400})
        for ev in (events or {}).get("events", []):
            course = (ev.get("course") or {}).get("fullnamedisplay") or course_names.get((ev.get("course") or {}).get("id", 0))
            updates.append({
                "kind": "deadline",
                "title": ev.get("name", "Event"),
                "course": course,
                "url": ev.get("url") or (ev.get("action") or {}).get("url"),
                "content_text": re.sub(r"<[^>]+>", " ", ev.get("description") or "").strip()[:500],
                "timestamp": datetime.fromtimestamp(ev["timesort"]) if ev.get("timesort") else None,
            })

        notifications = _ajax(client, sesskey, "message_popup_get_popup_notifications",
                              {"useridto": 0, "limit": 20, "offset": 0})
        for n in (notifications or {}).get("notifications", []):
            updates.append({
                "kind": "announcement",
                "title": n.get("subject") or "Notification",
                "course": None,
                "url": n.get("contexturl"),
                "content_text": re.sub(r"<[^>]+>", " ", n.get("smallmessage") or n.get("fullmessage") or "").strip()[:500],
                "timestamp": datetime.fromtimestamp(n["timecreated"]) if n.get("timecreated") else None,
            })

        return updates
