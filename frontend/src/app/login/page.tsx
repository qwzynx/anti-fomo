"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
import { api, setToken, User } from "../../lib/api";

const MAJORS = ["Software Engineering"];

interface AuthResponse {
  token: string;
  user: User;
}

interface YorkuStatus {
  status: "pending" | "success" | "failed";
  message: string;
  token?: string;
  user?: User;
}

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [name, setName] = useState("");
  const [major, setMajor] = useState(MAJORS[0]);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const [yorkuPhase, setYorkuPhase] = useState<"idle" | "pending" | "failed">("idle");
  const [yorkuMessage, setYorkuMessage] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, []);

  async function signInWithYorku() {
    setError(null);
    setYorkuPhase("pending");
    setYorkuMessage("Opening the official YorkU login window…");
    let attemptId: string;
    try {
      const res = await api<{ attempt_id: string }>("/api/auth/yorku/start", { method: "POST" });
      attemptId = res.attempt_id;
    } catch (err) {
      setYorkuPhase("failed");
      setYorkuMessage(err instanceof Error ? err.message : "Could not start the YorkU sign-in.");
      return;
    }

    if (pollRef.current) clearInterval(pollRef.current);
    pollRef.current = setInterval(async () => {
      try {
        const res = await api<YorkuStatus>(`/api/auth/yorku/status/${attemptId}`);
        if (res.status === "pending") {
          setYorkuMessage(res.message);
          return;
        }
        if (pollRef.current) clearInterval(pollRef.current);
        if (res.status === "success" && res.token) {
          setToken(res.token);
          router.push("/dashboard");
        } else {
          setYorkuPhase("failed");
          setYorkuMessage(res.message || "YorkU sign-in did not complete.");
        }
      } catch {
        // transient poll failure — keep waiting
      }
    }, 2000);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const body =
        mode === "login"
          ? { email, password }
          : { email, password, name, major };
      const res = await api<AuthResponse>(`/api/auth/${mode}`, {
        method: "POST",
        body: JSON.stringify(body),
      });
      setToken(res.token);
      router.push("/dashboard");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Something went wrong.");
    } finally {
      setSubmitting(false);
    }
  }

  const inputClass =
    "w-full px-4 py-2.5 bg-zinc-100 dark:bg-zinc-900 border border-transparent rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500 transition-all";

  return (
    <div className="flex min-h-screen flex-col bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <Header />

      <main className="flex flex-1 items-center justify-center px-6 py-16">
        <div className="animate-fade-up w-full max-w-md rounded-3xl border border-zinc-200 bg-white p-8 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
          <h2 className="text-2xl font-bold mb-1">
            {mode === "login" ? "Welcome back" : "Create your account"}
          </h2>
          <p className="text-sm text-zinc-500 dark:text-zinc-400 mb-6">
            {mode === "login"
              ? "Sign in to your student hub."
              : "Join to get a personalized student hub with eClass updates."}
          </p>

          {/* Primary: official YorkU Passport York sign-in (native Duo 2FA) */}
          {yorkuPhase === "pending" ? (
            <div className="mb-6 flex items-center gap-3 rounded-2xl border border-red-200 bg-red-50 p-4 dark:border-red-900/50 dark:bg-red-950/20">
              <div className="h-5 w-5 shrink-0 animate-spin rounded-full border-[3px] border-red-600 border-t-transparent" />
              <div>
                <p className="text-sm font-semibold text-red-700 dark:text-red-300">
                  {yorkuMessage ?? "Waiting for YorkU sign-in…"}
                </p>
                <p className="text-xs text-red-600/70 dark:text-red-400/70 mt-0.5">
                  Sign in on the YorkU window that opened. Approve the Duo push if prompted.
                </p>
              </div>
            </div>
          ) : (
            <button
              onClick={signInWithYorku}
              className="mb-3 w-full rounded-xl bg-red-700 py-3 font-semibold text-white transition-all hover:bg-red-600 hover:shadow-lg hover:shadow-red-700/25"
            >
              🎓 Sign in with YorkU Passport York
            </button>
          )}
          {yorkuPhase === "failed" && yorkuMessage && (
            <p className="mb-3 rounded-lg bg-red-50 px-4 py-2.5 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-300">
              {yorkuMessage}
            </p>
          )}
          <p className="mb-4 text-center text-[11px] text-zinc-400 leading-relaxed">
            Opens the official Passport York page — Duo 2FA works natively and
            Anti-FOMO never sees your password. Your eClass connects automatically.
          </p>

          <div className="mb-6 flex items-center gap-3 text-xs text-zinc-400">
            <span className="h-px flex-1 bg-zinc-200 dark:bg-zinc-800" />
            or continue with email
            <span className="h-px flex-1 bg-zinc-200 dark:bg-zinc-800" />
          </div>

          <div className="mb-6 grid grid-cols-2 rounded-lg bg-zinc-100 p-1 text-sm font-medium dark:bg-zinc-800">
            {(["login", "register"] as const).map((m) => (
              <button
                key={m}
                onClick={() => { setMode(m); setError(null); }}
                className={`rounded-md py-1.5 transition-colors ${
                  mode === m
                    ? "bg-white shadow-sm dark:bg-zinc-700"
                    : "text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
                }`}
              >
                {m === "login" ? "Sign in" : "Register"}
              </button>
            ))}
          </div>

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            {mode === "register" && (
              <>
                <input
                  type="text"
                  placeholder="Full name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  className={inputClass}
                />
                <select
                  value={major}
                  onChange={(e) => setMajor(e.target.value)}
                  className={inputClass}
                >
                  {MAJORS.map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
              </>
            )}
            <input
              type="email"
              required
              placeholder="Email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className={inputClass}
            />
            <input
              type="password"
              required
              minLength={mode === "register" ? 8 : undefined}
              placeholder={mode === "register" ? "Password (min. 8 characters)" : "Password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className={inputClass}
            />

            {error && (
              <p className="rounded-lg bg-red-50 px-4 py-2.5 text-sm text-red-700 dark:bg-red-900/20 dark:text-red-300">
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={submitting}
              className="mt-2 rounded-lg bg-indigo-600 py-2.5 font-semibold text-white transition-colors hover:bg-indigo-500 disabled:opacity-50"
            >
              {submitting
                ? "Please wait…"
                : mode === "login" ? "Sign in" : "Create account"}
            </button>
          </form>
        </div>
      </main>
    </div>
  );
}
