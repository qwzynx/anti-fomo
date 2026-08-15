"use client";

import { useId, useState } from "react";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
import Footer from "../../components/Footer";
import { AlertIcon } from "../../components/icons";
import { Dropdown } from "../../components/Dropdown";
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
  const ids = useId();

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

  // Inputs sit on the sunken surface with a visible border. They previously
  // used the same colour as the card they sit on plus a transparent border,
  // which made every field invisible in dark mode.
  const inputClass =
    "w-full rounded-lg border border-line bg-line-soft px-4 py-2.5 text-foreground transition-colors placeholder:text-subtle hover:border-line focus:outline-none";
  const labelClass = "mb-1.5 block text-xs font-semibold text-muted";

  return (
    <div className="flex min-h-screen flex-col bg-background font-sans text-foreground">
      <Header />

      <main className="flex flex-1 items-center justify-center px-6 py-16">
        <div className="animate-fade-up w-full max-w-md rounded-3xl border border-line bg-surface p-8">
          <h2 className="mb-1 text-2xl font-bold">
            {mode === "login" ? "Welcome back" : "Create your account"}
          </h2>
          <p className="mb-6 text-sm text-muted">
            {mode === "login"
              ? "Sign in to your student hub."
              : "Join to get a personalized student hub."}
          </p>

          <div
            role="tablist"
            aria-label="Authentication mode"
            className="mb-6 grid grid-cols-2 rounded-lg bg-line-soft p-1 text-sm font-medium"
          >
            {(["login", "register"] as const).map((m) => (
              <button
                key={m}
                role="tab"
                aria-selected={mode === m}
                onClick={() => { setMode(m); setError(null); }}
                className={`rounded-md py-1.5 transition-colors ${
                  mode === m
                    ? "bg-surface text-foreground"
                    : "text-subtle hover:text-foreground"
                }`}
              >
                {m === "login" ? "Sign in" : "Register"}
              </button>
            ))}
          </div>

          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            {mode === "register" && (
              <>
                <div>
                  <label htmlFor={`${ids}-name`} className={labelClass}>
                    Full name
                  </label>
                  <input
                    id={`${ids}-name`}
                    type="text"
                    autoComplete="name"
                    placeholder="Ada Lovelace"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    className={inputClass}
                  />
                </div>
                <div>
                  <span className={labelClass}>Major</span>
                  <Dropdown
                    label="Major"
                    value={major}
                    onChange={setMajor}
                    options={MAJORS.map((m) => ({ value: m, label: m }))}
                  />
                </div>
              </>
            )}

            <div>
              <label htmlFor={`${ids}-email`} className={labelClass}>
                Email
              </label>
              <input
                id={`${ids}-email`}
                type="email"
                required
                autoComplete="email"
                placeholder="you@university.edu"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className={inputClass}
              />
            </div>

            <div>
              <label htmlFor={`${ids}-password`} className={labelClass}>
                Password
              </label>
              <input
                id={`${ids}-password`}
                type="password"
                required
                minLength={mode === "register" ? 8 : undefined}
                autoComplete={mode === "register" ? "new-password" : "current-password"}
                placeholder={mode === "register" ? "At least 8 characters" : "Your password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className={inputClass}
              />
            </div>

            {error && (
              <p
                role="alert"
                className="flex items-start gap-2 rounded-lg bg-danger-soft px-4 py-2.5 text-sm text-danger"
              >
                <AlertIcon className="mt-0.5 h-4 w-4 shrink-0" />
                {error}
              </p>
            )}

            <button
              type="submit"
              disabled={submitting}
              className="mt-2 rounded-lg bg-brand py-2.5 font-semibold text-brand-fg transition-colors hover:bg-brand-hover disabled:opacity-50"
            >
              {submitting
                ? "Please wait…"
                : mode === "login" ? "Sign in" : "Create account"}
            </button>
          </form>
        </div>
      </main>

      <Footer />
    </div>
  );
}
