"use client";

import { useEffect, useState } from "react";
import Image from "next/image";

interface ScrapedItem {
  title: string;
  source_platform: string;
  item_type: string;
  url: string;
  content_text: string;
  timestamp: string;
  discipline: string;
  relevance_score: number;
}

export default function Home() {
  const [items, setItems] = useState<ScrapedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [major, setMajor] = useState("Software Engineering");

  useEffect(() => {
    async function fetchFeed() {
      setLoading(true);
      try {
        const response = await fetch(`http://localhost:8000/api/feed?major=${encodeURIComponent(major)}`);
        const data = await response.json();
        setItems(data);
      } catch (error) {
        console.error("Failed to fetch feed:", error);
      } finally {
        setLoading(false);
      }
    }

    fetchFeed();
  }, [major]);

  return (
    <div className="flex flex-col min-h-screen bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <header className="sticky top-0 z-10 w-full border-b border-zinc-200 bg-white/80 backdrop-blur-md dark:border-zinc-800 dark:bg-black/80">
        <div className="mx-auto flex max-w-5xl items-center justify-between p-4">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white font-bold">AF</div>
            <h1 className="text-xl font-bold tracking-tight">Anti-FOMO</h1>
          </div>
          <div className="flex gap-4 items-center text-sm font-medium">
             <select 
              value={major} 
              onChange={(e) => setMajor(e.target.value)}
              className="bg-transparent border border-zinc-300 dark:border-zinc-700 rounded-md px-2 py-1 outline-none focus:ring-2 focus:ring-indigo-500"
            >
              <option value="Software Engineering">Software Engineering</option>
              <option value="Mechanical Engineering">Mechanical Engineering</option>
              <option value="Civil Engineering">Civil Engineering</option>
              <option value="Business">Business</option>
            </select>
          </div>
        </div>
      </header>

      <main className="flex-1 mx-auto w-full max-w-3xl py-12 px-6">
        <div className="mb-8">
          <h2 className="text-3xl font-bold mb-2">Your Opportunities</h2>
          <p className="text-zinc-500 dark:text-zinc-400">Personalized feed for {major} students.</p>
        </div>

        {loading ? (
          <div className="flex flex-col gap-4">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-32 w-full animate-pulse rounded-xl bg-zinc-200 dark:bg-zinc-800" />
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-6">
            {items.map((item, idx) => (
              <a
                key={idx}
                href={item.url}
                target="_blank"
                rel="noopener noreferrer"
                className="group relative flex flex-col gap-2 rounded-2xl border border-zinc-200 bg-white p-6 shadow-sm transition-all hover:border-indigo-500 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-400"
              >
                <div className="flex items-start justify-between gap-4">
                  <div className="flex flex-col gap-1">
                    <span className="text-xs font-bold uppercase tracking-wider text-indigo-600 dark:text-indigo-400">
                      {item.source_platform} • {item.item_type}
                    </span>
                    <h3 className="text-xl font-semibold leading-tight group-hover:text-indigo-600 dark:group-hover:text-indigo-400">
                      {item.title}
                    </h3>
                  </div>
                  {item.relevance_score > 10 && (
                    <span className="flex-shrink-0 rounded-full bg-indigo-100 px-2 py-1 text-[10px] font-bold text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300">
                      TOP MATCH
                    </span>
                  )}
                </div>
                <p className="line-clamp-2 text-sm leading-relaxed text-zinc-600 dark:text-zinc-400">
                  {item.content_text || "No description provided."}
                </p>
                <div className="mt-2 flex items-center gap-4 text-xs font-medium text-zinc-400">
                  <span>{item.discipline}</span>
                  <span>{new Date(item.timestamp).toLocaleDateString()}</span>
                </div>
              </a>
            ))}
          </div>
        )}
      </main>

      <footer className="mt-auto border-t border-zinc-200 py-8 text-center text-sm text-zinc-500 dark:border-zinc-800">
        <p>© 2026 Anti-FOMO. Data aggregated from Y Combinator, Phoronix, YorkU, and SimplifyJobs.</p>
      </footer>
    </div>
  );
}
