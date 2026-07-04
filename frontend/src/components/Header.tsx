"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { getToken } from "../lib/api";

const NAV = [
  { href: "/", label: "Feed", key: "feed" },
  { href: "/internships", label: "Internships", key: "internships" },
] as const;

export default function Header({
  active,
  children,
}: {
  active?: "feed" | "internships" | "hub";
  children?: React.ReactNode;
}) {
  const [signedIn, setSignedIn] = useState(false);

  useEffect(() => {
    setSignedIn(getToken() !== null);
  }, []);

  return (
    <header className="sticky top-0 z-20 w-full border-b border-zinc-200 glass dark:border-zinc-800">
      <div className="mx-auto flex max-w-6xl items-center justify-between gap-4 p-4 flex-wrap">
        <Link href="/" className="flex items-center gap-2 shrink-0">
          <div className="h-8 w-8 rounded-lg bg-gradient-to-br from-indigo-500 to-violet-600 flex items-center justify-center text-white font-bold shadow-sm">
            AF
          </div>
          <h1 className="text-xl font-bold tracking-tight">Anti-FOMO</h1>
        </Link>

        <nav className="flex items-center gap-1 text-sm font-medium">
          {NAV.map((n) => (
            <Link
              key={n.key}
              href={n.href}
              className={`rounded-full px-3.5 py-1.5 transition-colors ${
                active === n.key
                  ? "bg-indigo-600/10 text-indigo-600 dark:bg-indigo-400/10 dark:text-indigo-400"
                  : "text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100"
              }`}
            >
              {n.label}
            </Link>
          ))}
          <Link
            href={signedIn ? "/dashboard" : "/login"}
            className={`ml-2 rounded-full px-4 py-1.5 font-semibold transition-all ${
              active === "hub"
                ? "bg-indigo-600 text-white"
                : "bg-indigo-600 text-white hover:bg-indigo-500 hover:shadow-md hover:shadow-indigo-600/20"
            }`}
          >
            {signedIn ? "Student Hub" : "Sign in"}
          </Link>
        </nav>

        {children && <div className="w-full md:w-auto md:flex-1 md:order-none order-last">{children}</div>}
      </div>
    </header>
  );
}
