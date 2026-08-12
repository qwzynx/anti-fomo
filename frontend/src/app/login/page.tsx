"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
import { api, setToken, User } from "../../lib/api";

const MAJORS = ["Software Engineering"];

interface AuthResponse {
  token: string;
  user: User;
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
              : "Join to get a personalized student hub."}
          </p>

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
