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
 * Applies the theme before the first paint.
 *
 * Without this the page renders in one theme and snaps to the user's choice
 * once React hydrates — a visible flash. Falls back to the OS preference on a
 * first visit, then the stored choice wins forever after. Kept in sync with
 * `readTheme`/`applyTheme` in ThemeToggle.tsx.
 */
const themeBootScript = `
(function () {
  try {
    var t = localStorage.getItem("antifomo_theme");
    if (t !== "light" && t !== "dark") {
      t = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    if (t === "dark") document.documentElement.classList.add("dark");
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
      <body className="min-h-full flex flex-col bg-background text-foreground">{children}</body>
    </html>
  );
}
