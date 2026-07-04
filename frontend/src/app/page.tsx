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

const DISCIPLINES = ["All", "Software Engineering", "Mechanical Engineering", "Civil Engineering", "Business"];

export default function Home() {
  const [items, setItems] = useState<ScrapedItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [major, setMajor] = useState("Software Engineering");
  const [searchTerm, setSearchTerm] = useState("");
  const [selectedDiscipline, setSelectedDiscipline] = useState("All");

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

  const filteredItems = items.filter(item => {
    const matchesSearch = item.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
                          (item.content_text && item.content_text.toLowerCase().includes(searchTerm.toLowerCase()));
    const matchesDiscipline = selectedDiscipline === 'All' || item.discipline === selectedDiscipline;
    return matchesSearch && matchesDiscipline;
  });

  return (
    <div className="flex flex-col min-h-screen bg-zinc-50 font-sans dark:bg-black text-zinc-900 dark:text-zinc-100">
      <header className="sticky top-0 z-10 w-full border-b border-zinc-200 bg-white/80 backdrop-blur-md dark:border-zinc-800 dark:bg-black/80">
        <div className="mx-auto flex max-w-5xl items-center justify-between p-4 flex-wrap gap-4">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white font-bold">AF</div>
            <h1 className="text-xl font-bold tracking-tight">Anti-FOMO</h1>
          </div>
          
          <div className="flex-1 min-w-[300px]">
            <input
              type="text"
              placeholder="Search opportunities..."
              className="w-full px-4 py-2 bg-zinc-100 dark:bg-zinc-900 border border-transparent rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500 transition-all"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>

          <div className="flex gap-4 items-center text-sm font-medium">
             <span className="text-zinc-500">Major:</span>
             <select 
              value={major} 
              onChange={(e) => setMajor(e.target.value)}
              className="bg-transparent border border-zinc-300 dark:border-zinc-700 rounded-md px-2 py-1 outline-none focus:ring-2 focus:ring-indigo-500"
            >
              {DISCIPLINES.filter(d => d !== "All").map(d => (
                <option key={d} value={d}>{d}</option>
              ))}
            </select>
          </div>
        </div>
        
        <div className="bg-white dark:bg-black px-4 py-2 border-t border-zinc-100 dark:border-zinc-900 overflow-x-auto scrollbar-hide">
          <div className="mx-auto max-w-5xl flex gap-2">
            {DISCIPLINES.map(discipline => (
              <button
                key={discipline}
                onClick={() => setSelectedDiscipline(discipline)}
                className={`px-3 py-1 rounded-full text-xs font-medium whitespace-nowrap transition-colors ${ 
                  selectedDiscipline === discipline
                    ? 'bg-indigo-600 text-white'
                    : 'bg-zinc-100 dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-800'
                }`}
              >
                {discipline}
              </button>
            ))}
          </div>
        </div>
      </header>

      <main className="flex-1 mx-auto w-full max-w-5xl py-12 px-6">
        <div className="mb-8 flex flex-col md:flex-row md:items-end justify-between gap-4">
          <div>
            <h2 className="text-3xl font-bold mb-2">Your Opportunities</h2>
            <p className="text-zinc-500 dark:text-zinc-400">
              Showing {filteredItems.length} results for {major} students
              {selectedDiscipline !== "All" && ` filtered by ${selectedDiscipline}`}.
            </p>
          </div>
        </div>

        {loading ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {[1, 2, 3, 4, 5, 6].map((i) => (
              <div key={i} className="h-48 w-full animate-pulse rounded-2xl bg-zinc-200 dark:bg-zinc-800" />
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {filteredItems.map((item, idx) => (
              <a
                key={idx}
                href={item.url}
                target="_blank"
                rel="noopener noreferrer"
                className="group relative flex flex-col gap-3 rounded-2xl border border-zinc-200 bg-white p-6 shadow-sm transition-all hover:border-indigo-500 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900 dark:hover:border-indigo-400"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex flex-col gap-1">
                    <span className={`text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded-full ${
                      item.item_type === 'Internship' || item.item_type === 'Job'
                        ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300'
                        : item.item_type === 'Event'
                          ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-300'
                          : 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/30 dark:text-indigo-300'
                    }`}>
                      {item.item_type}
                    </span>
                  </div>
                  {item.relevance_score > 10 && (
                    <span className="flex-shrink-0 rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-bold text-amber-700 dark:bg-amber-900/30 dark:text-amber-300">
                      TOP MATCH
                    </span>
                  )}
                </div>
                
                <div className="flex flex-col gap-1">
                  <h3 className="text-lg font-bold leading-tight group-hover:text-indigo-600 dark:group-hover:text-indigo-400 line-clamp-2">
                    {item.title}
                  </h3>
                  <span className="text-xs text-zinc-400 font-medium">{item.source_platform}</span>
                </div>

                <p className="line-clamp-3 text-sm leading-relaxed text-zinc-600 dark:text-zinc-400 flex-1">
                  {item.content_text || "No description provided."}
                </p>

                <div className="mt-auto pt-4 border-t border-zinc-100 dark:border-zinc-800 flex items-center justify-between text-[10px] font-medium text-zinc-400 uppercase tracking-tight">
                  <span>{item.discipline}</span>
                  <span>{new Date(item.timestamp).toLocaleDateString()}</span>
                </div>
              </a>
            ))}
          </div>
        )}

        {!loading && filteredItems.length === 0 && (
          <div className="text-center py-20 bg-white dark:bg-zinc-900 rounded-3xl border border-dashed border-zinc-300 dark:border-zinc-700">
            <p className="text-zinc-500 dark:text-zinc-400">No opportunities match your search or filters.</p>
            <button 
              onClick={() => {setSearchTerm(""); setSelectedDiscipline("All");}}
              className="mt-4 text-indigo-600 font-semibold text-sm hover:underline"
            >
              Clear all filters
            </button>
          </div>
        )}
      </main>

      <footer className="mt-auto border-t border-zinc-200 py-8 text-center text-sm text-zinc-500 dark:border-zinc-800">
        <p>© 2026 Anti-FOMO. Data aggregated from Y Combinator, Phoronix, YorkU, and SimplifyJobs.</p>
      </footer>
    </div>
  );
}
