"""
Scraper for York University's eClass (Moodle) portal.

Login goes through YorkU's Shibboleth SSO (shib.yorku.ca) with Duo 2FA, so it
is driven with Playwright. Credentials are used transiently for the login and
are never written to disk or the database — only the resulting session
cookies (Playwright storage state) are persisted, per user. Data is then
pulled through Moodle's session-authenticated AJAX API with plain httpx,
which is far faster and more stable than DOM scraping.
"""

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

# Generous timeout: the student may need to approve a Duo push on their phone.
LOGIN_TIMEOUT_SECONDS = 180

SESSIONS_DIR = Path(__file__).parent / "data" / "eclass_sessions"

USERNAME_SELECTORS = "#username, input[name='j_username'], input[name='mli'], input[name='username']"
PASSWORD_SELECTORS = "#password, input[name='j_password'], input[name='password'], input[type='password']"
SUBMIT_SELECTORS = "button[type='submit'], input[type='submit'], input[name='_eventId_proceed']"


class EclassSessionExpired(Exception):
    """Saved eClass session no longer works; the student must re-link."""


# Status of in-flight interactive (popup) link attempts, keyed by user id.
# Single-process app, so a module-level dict is sufficient.
link_attempts: Dict[int, Dict[str, str]] = {}


async def link_account_interactive(user_id: int, state_path: Path) -> None:
    """
    Opens a real (headed) browser window on the official YorkU login portal.
    The student types their credentials directly into York's page — including
    any Duo 2FA step — and we only capture the resulting session cookies.
    Progress is reported through `link_attempts[user_id]`.
    """
    link_attempts[user_id] = {"status": "pending", "message": "Waiting for you to sign in to YorkU…"}
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
                        link_attempts[user_id] = {"status": "failed", "message": "The login window was closed before finishing."}
                        return
                    url = page.url
                    if url.startswith(ECLASS_BASE) and "/login" not in url:
                        await context.storage_state(path=str(state_path))
                        os.chmod(state_path, 0o600)
                        await browser.close()
                        link_attempts[user_id] = {"status": "success", "message": "eClass account linked."}
                        return
                    await page.wait_for_timeout(1000)
                except Exception:
                    link_attempts[user_id] = {"status": "failed", "message": "The login window was closed before finishing."}
                    return

            await browser.close()
            link_attempts[user_id] = {"status": "failed", "message": "Timed out waiting for the YorkU login to finish."}
    except Exception as e:
        link_attempts[user_id] = {"status": "failed", "message": f"Could not open the login window: {e}"}


def state_path_for(user_id: int) -> Path:
    SESSIONS_DIR.mkdir(parents=True, exist_ok=True)
    return SESSIONS_DIR / f"user_{user_id}.json"


async def link_account(username: str, password: str, state_path: Path) -> Dict[str, Any]:
    """
    Logs into eClass via Passport York/Shibboleth and saves the session state.
    Returns {"success": bool, "message": str}.
    """
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context()
        page = await context.new_page()
        try:
            await page.goto(LOGIN_URL, timeout=30000)

            # Shibboleth shows a JS localStorage bounce page first; just wait
            # until a username field appears anywhere in the chain.
            await page.wait_for_selector(USERNAME_SELECTORS, timeout=30000)
            await page.fill(USERNAME_SELECTORS, username)
            # Some flows ask for username first, then password on submit.
            pw = page.locator(PASSWORD_SELECTORS)
            if await pw.count() > 0:
                await pw.first.fill(password)
            await page.locator(SUBMIT_SELECTORS).first.click()

            deadline = time.monotonic() + LOGIN_TIMEOUT_SECONDS
            while time.monotonic() < deadline:
                url = page.url
                if url.startswith(ECLASS_BASE) and "/login" not in url:
                    await context.storage_state(path=str(state_path))
                    os.chmod(state_path, 0o600)
                    return {"success": True, "message": "eClass account linked."}

                content = await page.content()
                lowered = content.lower()
                if "password" in lowered and any(
                    err in lowered for err in ("incorrect", "cannot be identified", "invalid", "unable to log in")
                ):
                    return {"success": False, "message": "eClass rejected the username or password."}

                # Duo universal prompt: press "Yes, this is my device" if shown
                # so future logins can reuse the session longer.
                trust = page.locator("#trust-browser-button")
                if await trust.count() > 0:
                    try:
                        await trust.first.click(timeout=2000)
                    except Exception:
                        pass

                # A second password field can appear after a username-only step.
                pw = page.locator(PASSWORD_SELECTORS)
                if await pw.count() > 0 and await pw.first.input_value() == "":
                    await pw.first.fill(password)
                    await page.locator(SUBMIT_SELECTORS).first.click()

                await page.wait_for_timeout(2000)

            return {
                "success": False,
                "message": "Timed out waiting for login — if you use Duo, approve the push and try again.",
            }
        except Exception as e:
            return {"success": False, "message": f"Login automation failed: {e}"}
        finally:
            await browser.close()


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
