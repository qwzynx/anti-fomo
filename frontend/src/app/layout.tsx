import type { Metadata, Viewport } from "next";
import { Inter, Outfit } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const outfit = Outfit({
  variable: "--font-outfit",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Anti-FOMO — Your Student Hub",
  description:
    "Internships, tech news, and campus events in one personalized feed.",
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f6f6f7" },
    { media: "(prefers-color-scheme: dark)", color: "#09090b" },
  ],
};

/**
 * Applies the stored theme before the first paint.
 *
 * Without this the page renders in the OS theme and then snaps to the user's
 * choice once React hydrates — a visible white flash for anyone who picked
 * dark on a light system. Kept in sync with `applyTheme` in ThemeToggle.tsx.
 */
const themeBootScript = `
(function () {
  try {
    var t = localStorage.getItem("antifomo_theme");
    if (t === "light" || t === "dark") {
      document.documentElement.setAttribute("data-theme", t);
    }
  } catch (e) {}
})();
`;

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${outfit.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeBootScript }} />
      </head>
      <body className="min-h-full flex flex-col bg-page text-ink">{children}</body>
    </html>
  );
}
