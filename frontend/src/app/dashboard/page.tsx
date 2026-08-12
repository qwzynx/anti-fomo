"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Header from "../../components/Header";
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

        <section className="animate-fade-up max-w-2xl rounded-3xl border border-zinc-200 bg-white p-8 dark:border-zinc-800 dark:bg-zinc-900">
          <h3 className="text-xl font-bold mb-1">Your feed is ready</h3>
          <p className="text-sm text-zinc-500 dark:text-zinc-400 leading-relaxed">
            Head back to the home feed to browse internships, tech news, and
            events curated for {user.major} students.
          </p>
        </section>
      </main>
    </div>
  );
}
