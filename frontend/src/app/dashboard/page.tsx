"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
import { api, ApiError, EclassUpdate, getToken, setToken, User } from "../../lib/api";

interface UpdatesResponse {
  fetched_at: string | null;
  updates: EclassUpdate[];
}

interface LinkStatus {
  status: "none" | "pending" | "success" | "failed";
  message: string;
  user?: User;
}

export default function DashboardPage() {
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);
  const [updates, setUpdates] = useState<EclassUpdate[]>([]);
  const [fetchedAt, setFetchedAt] = useState<string | null>(null);
  const [loadingUpdates, setLoadingUpdates] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Popup link flow
  const [linkPhase, setLinkPhase] = useState<"idle" | "pending" | "failed">("idle");
  const [linkMessage, setLinkMessage] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loadUpdates = useCallback(async (refresh: boolean) => {
    setLoadingUpdates(true);
    setError(null);
    try {
      const res = await api<UpdatesResponse>(
        `/api/eclass/updates${refresh ? "?refresh=true" : ""}`
      );
      setUpdates(res.updates);
      setFetchedAt(res.fetched_at);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        // Session with eClass expired — needs re-link (distinct from app auth).
        setUser((u) => (u ? { ...u, eclass_linked: false } : u));
      }
      setError(err instanceof Error ? err.message : "Failed to load updates.");
    } finally {
      setLoadingUpdates(false);
    }
  }, []);

  useEffect(() => {
    if (!getToken()) {
      router.replace("/login");
      return;
    }
    api<User>("/api/auth/me")
      .then((u) => {
        setUser(u);
        if (u.eclass_linked) loadUpdates(false);
      })
      .catch(() => {
        setToken(null);
        router.replace("/login");
      });
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [router, loadUpdates]);

  async function startPopupLink() {
    setError(null);
    setLinkPhase("pending");
    setLinkMessage("Opening the official YorkU login window…");
    try {
      await api<LinkStatus>("/api/eclass/link/interactive", { method: "POST" });
    } catch (err) {
      setLinkPhase("failed");
      setLinkMessage(err instanceof Error ? err.message : "Could not start the login.");
      return;
    }

    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const res = await api<LinkStatus>("/api/eclass/link/status");
        if (res.status === "pending") {
          setLinkMessage(res.message);
          return;
        }
        if (pollRef.current) clearInterval(pollRef.current);
        if (res.status === "success" && res.user) {
          setLinkPhase("idle");
          setLinkMessage(null);
          setUser(res.user);
          loadUpdates(true);
        } else {
          setLinkPhase("failed");
          setLinkMessage(res.message || "Login did not complete.");
        }
      } catch {
        // transient poll failure — keep waiting
      }
    }, 2000);
  }

  async function handleUnlink() {
    await api("/api/eclass/link", { method: "DELETE" }).catch(() => {});
    setUpdates([]);
    setLinkPhase("idle");
    setLinkMessage(null);
    setUser((u) => (u ? { ...u, eclass_linked: false } : u));
  }

  function handleLogout() {
    setToken(null);
    router.push("/");
  }

  const deadlines = updates
    .filter((u) => u.kind === "deadline")
    .sort((a, b) => (a.timestamp ?? "").localeCompare(b.timestamp ?? ""));
  const announcements = updates
    .filter((u) => u.kind === "announcement")
    .sort((a, b) => (b.timestamp ?? "").localeCompare(a.timestamp ?? ""));
  const courses = updates.filter((u) => u.kind === "course");

  if (!user) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-zinc-50 dark:bg-black">
        <div className="h-10 w-10 animate-spin rounded-full border-4 border-indigo-600 border-t-transparent" />
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <Header active="hub" />

      <main className="mx-auto w-full max-w-6xl flex-1 px-6 py-10">
        <div className="mb-8 flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="text-3xl font-bold">
              Hi{user.name ? `, ${user.name.split(" ")[0]}` : ""} 👋
            </h2>
            <p className="text-zinc-500 dark:text-zinc-400">
              {user.major} · {user.email}
            </p>
          </div>
          <button
            onClick={handleLogout}
            className="rounded-xl border border-zinc-300 px-4 py-2 text-sm font-medium hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-900"
          >
            Sign out
          </button>
        </div>

        {error && (
          <p className="mb-6 rounded-xl bg-red-50 px-4 py-3 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-300">
            {error}
          </p>
        )}

        {!user.eclass_linked ? (
          <section className="animate-fade-up max-w-2xl rounded-3xl border border-zinc-200 bg-white p-8 dark:border-zinc-800 dark:bg-zinc-900">
            <h3 className="text-xl font-bold mb-1">Connect YorkU eClass</h3>
            <p className="text-sm text-zinc-500 dark:text-zinc-400 mb-6 leading-relaxed">
              A secure window will open with the <strong>official YorkU Passport York
              login page</strong>. Sign in there — including your Duo 2FA step — exactly
              like you normally would. Anti-FOMO never sees or stores your password;
              once York confirms the login, the window closes and your courses,
              deadlines, and announcements appear here.
            </p>

            {linkPhase === "pending" ? (
              <div className="flex items-center gap-4 rounded-2xl border border-indigo-200 bg-indigo-50 p-5 dark:border-indigo-900/50 dark:bg-indigo-950/30">
                <div className="h-6 w-6 shrink-0 animate-spin rounded-full border-[3px] border-indigo-600 border-t-transparent" />
                <div>
                  <p className="font-semibold text-indigo-700 dark:text-indigo-300">
                    {linkMessage ?? "Waiting for you to sign in…"}
                  </p>
                  <p className="text-xs text-indigo-600/70 dark:text-indigo-400/70 mt-0.5">
                    Complete the login in the YorkU window. Keep your phone nearby for the Duo push.
                  </p>
                </div>
              </div>
            ) : (
              <>
                <button
                  onClick={startPopupLink}
                  className="w-full rounded-xl bg-indigo-600 py-3 font-semibold text-white transition-all hover:bg-indigo-500 hover:shadow-lg hover:shadow-indigo-600/25 sm:w-auto sm:px-8"
                >
                  🔐 Connect with Passport York
                </button>
                {linkPhase === "failed" && linkMessage && (
                  <p className="mt-3 rounded-xl bg-red-50 px-4 py-2.5 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-300">
                    {linkMessage}
                  </p>
                )}
              </>
            )}

            <p className="mt-5 text-xs text-zinc-400 leading-relaxed">
              🔒 Zero credential handling: the sign-in happens entirely on York
              University's official pages, so Duo push, passcodes, and security
              keys all work exactly as they do on eClass itself.
            </p>
          </section>
        ) : (
          <>
            <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
              <p className="text-sm text-zinc-500 dark:text-zinc-400">
                ✅ eClass connected
                {fetchedAt && ` · last updated ${new Date(fetchedAt).toLocaleString()}`}
              </p>
              <div className="flex gap-2">
                <button
                  onClick={() => loadUpdates(true)}
                  disabled={loadingUpdates}
                  className="rounded-xl bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
                >
                  {loadingUpdates ? "Refreshing…" : "Refresh"}
                </button>
                <button
                  onClick={handleUnlink}
                  className="rounded-xl border border-zinc-300 px-4 py-2 text-sm font-medium hover:bg-zinc-100 dark:border-zinc-700 dark:hover:bg-zinc-900"
                >
                  Unlink
                </button>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
              <section className="animate-fade-up rounded-3xl border border-zinc-200 bg-white p-6 dark:border-zinc-800 dark:bg-zinc-900">
                <h3 className="mb-4 text-lg font-bold">📅 Upcoming deadlines</h3>
                {deadlines.length === 0 ? (
                  <p className="text-sm text-zinc-500">Nothing due — enjoy the calm.</p>
                ) : (
                  <ul className="flex flex-col gap-3">
                    {deadlines.map((d, i) => (
                      <li key={i} className="rounded-xl bg-zinc-50 p-4 dark:bg-zinc-800/60">
                        <a href={d.url ?? "#"} target="_blank" rel="noopener noreferrer" className="font-semibold hover:text-indigo-600 dark:hover:text-indigo-400">
                          {d.title}
                        </a>
                        <p className="mt-1 text-xs text-zinc-500">
                          {d.course}
                          {d.timestamp && ` · due ${new Date(d.timestamp).toLocaleString()}`}
                        </p>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="animate-fade-up rounded-3xl border border-zinc-200 bg-white p-6 dark:border-zinc-800 dark:bg-zinc-900" style={{ animationDelay: "60ms" }}>
                <h3 className="mb-4 text-lg font-bold">📣 Announcements</h3>
                {announcements.length === 0 ? (
                  <p className="text-sm text-zinc-500">No recent announcements.</p>
                ) : (
                  <ul className="flex flex-col gap-3">
                    {announcements.map((a, i) => (
                      <li key={i} className="rounded-xl bg-zinc-50 p-4 dark:bg-zinc-800/60">
                        <a href={a.url ?? "#"} target="_blank" rel="noopener noreferrer" className="font-semibold hover:text-indigo-600 dark:hover:text-indigo-400">
                          {a.title}
                        </a>
                        {a.content_text && (
                          <p className="mt-1 line-clamp-2 text-sm text-zinc-600 dark:text-zinc-400">{a.content_text}</p>
                        )}
                        {a.timestamp && (
                          <p className="mt-1 text-xs text-zinc-500">{new Date(a.timestamp).toLocaleString()}</p>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section className="animate-fade-up rounded-3xl border border-zinc-200 bg-white p-6 dark:border-zinc-800 dark:bg-zinc-900 lg:col-span-2" style={{ animationDelay: "120ms" }}>
                <h3 className="mb-4 text-lg font-bold">📚 Your courses</h3>
                {courses.length === 0 ? (
                  <p className="text-sm text-zinc-500">No in-progress courses found.</p>
                ) : (
                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                    {courses.map((c, i) => (
                      <a
                        key={i}
                        href={c.url ?? "#"}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="rounded-xl border border-zinc-200 p-4 transition-colors hover:border-indigo-500 dark:border-zinc-700 dark:hover:border-indigo-400"
                      >
                        <p className="font-semibold leading-tight">{c.title}</p>
                        {c.course && <p className="mt-1 text-xs text-zinc-500">{c.course}</p>}
                      </a>
                    ))}
                  </div>
                )}
              </section>
            </div>
          </>
        )}
      </main>
    </div>
  );
}
