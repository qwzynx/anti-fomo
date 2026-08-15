"use client";

import Link from "next/link";
import { TOKEN_KEY } from "../lib/api";
import { useStoredValue } from "../lib/browserStore";
import ThemeToggle from "./ThemeToggle";

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
  // Subscribed rather than read once in an effect, so signing in or out
  // updates the header immediately instead of on the next full page load.
  const signedIn = useStoredValue(TOKEN_KEY, "") !== "";

  return (
    <header className="glass sticky top-0 z-20 w-full border-b border-line">
      <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-6 py-3.5">
        <Link href="/" className="flex shrink-0 items-center gap-2.5">
          <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-brand to-violet-600 text-sm font-bold text-brand-fg">
            AF
          </span>
          <span className="font-display text-lg font-bold tracking-tight">Anti-FOMO</span>
        </Link>

        <div className="flex items-center gap-2">
          <nav aria-label="Primary" className="flex items-center gap-1 text-sm font-medium">
            {NAV.map((n) => (
              <Link
                key={n.key}
                href={n.href}
                aria-current={active === n.key ? "page" : undefined}
                className={`rounded-full px-3.5 py-1.5 transition-colors ${
                  active === n.key
                    ? "bg-brand-soft text-brand-soft-fg"
                    : "text-muted hover:bg-line-soft hover:text-foreground"
                }`}
              >
                {n.label}
              </Link>
            ))}
          </nav>

          <ThemeToggle />

          <Link
            href={signedIn ? "/dashboard" : "/login"}
            aria-current={active === "hub" ? "page" : undefined}
            className="rounded-full bg-brand px-4 py-1.5 text-sm font-semibold text-brand-fg transition-colors hover:bg-brand-hover"
          >
            {signedIn ? "Student Hub" : "Sign in"}
          </Link>
        </div>

        {children && <div className="order-last w-full md:order-none md:w-auto md:flex-1">{children}</div>}
      </div>
    </header>
  );
}
