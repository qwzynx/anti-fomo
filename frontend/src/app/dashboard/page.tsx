"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
import Footer from "../../components/Footer";
import { ArrowRightIcon, BriefcaseIcon, NewspaperIcon } from "../../components/icons";
import { api, getToken, setToken, User } from "../../lib/api";

export default function DashboardPage() {
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.replace("/login");
      return;
    }
    api<User>("/api/auth/me")
      .then(setUser)
      .catch(() => {
        setToken(null);
        router.replace("/login");
      });
  }, [router]);

  function handleLogout() {
    setToken(null);
    router.push("/");
  }

  if (!user) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-page">
        <div
          role="status"
          aria-label="Loading your hub"
          className="h-10 w-10 animate-spin rounded-full border-4 border-brand border-t-transparent"
        />
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col bg-page font-sans text-ink">
      <Header active="hub" />

      <main className="mx-auto w-full max-w-6xl flex-1 px-6 py-10">
        <div className="mb-8 flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 className="text-3xl font-bold">
              Hi{user.name ? `, ${user.name.split(" ")[0]}` : ""}
            </h2>
            <p className="mt-1 text-sm text-ink-2">
              {user.major} · {user.email}
            </p>
          </div>
          <button
            onClick={handleLogout}
            className="rounded-xl border border-line px-4 py-2 text-sm font-medium transition-colors hover:bg-sunken"
          >
            Sign out
          </button>
        </div>

        <section className="grid gap-4 md:grid-cols-2">
          <Link
            href="/"
            className="animate-fade-up group flex flex-col rounded-2xl border border-line bg-card p-6 transition-all hover:-translate-y-0.5 hover:border-brand hover:shadow-[var(--shadow-lift)]"
          >
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-news-soft text-news-ink">
              <NewspaperIcon className="h-5 w-5" />
            </span>
            <h3 className="mt-4 text-lg font-bold">Your feed</h3>
            <p className="mt-1 text-sm leading-relaxed text-ink-2">
              Internships, tech news and events ranked for {user.major} students.
            </p>
            <span className="mt-4 flex items-center gap-1.5 text-sm font-semibold text-brand-ink">
              Open feed
              <ArrowRightIcon className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
            </span>
          </Link>

          <Link
            href="/internships"
            className="animate-fade-up group flex flex-col rounded-2xl border border-line bg-card p-6 transition-all hover:-translate-y-0.5 hover:border-brand hover:shadow-[var(--shadow-lift)]"
            style={{ animationDelay: "60ms" }}
          >
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-job-soft text-job-ink">
              <BriefcaseIcon className="h-5 w-5" />
            </span>
            <h3 className="mt-4 text-lg font-bold">Internship hub</h3>
            <p className="mt-1 text-sm leading-relaxed text-ink-2">
              Every open role, filterable by specialty, work mode and location.
            </p>
            <span className="mt-4 flex items-center gap-1.5 text-sm font-semibold text-brand-ink">
              Browse roles
              <ArrowRightIcon className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
            </span>
          </Link>
        </section>
      </main>

      <Footer />
    </div>
  );
}
