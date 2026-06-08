'use client';

import { useState, useEffect } from 'react';

const DISCIPLINES = ["All", "Software Engineering", "Mechanical Engineering", "Civil Engineering", "Business"];

export default function Home() {
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedDiscipline, setSelectedDiscipline] = useState('All');
  const [items, setItems] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchFeed() {
      try {
        setLoading(true);
        // Using Software Engineering as the default major for now
        const response = await fetch('http://127.0.0.1:8000/api/feed?major=Software+Engineering');
        if (!response.ok) {
          throw new Error('Failed to fetch feed');
        }
        const data = await response.json();
        setItems(data);
        setError(null);
      } catch (err) {
        console.error('Error fetching feed:', err);
        setError('Could not connect to the backend server. Make sure it is running on port 8000.');
      } finally {
        setLoading(false);
      }
    }

    fetchFeed();
  }, []);

  const filteredItems = items.filter(item => {
    const matchesSearch = item.title.toLowerCase().includes(searchTerm.toLowerCase()) || 
                          (item.content_text && item.content_text.toLowerCase().includes(searchTerm.toLowerCase()));
    const matchesDiscipline = selectedDiscipline === 'All' || item.discipline === selectedDiscipline;
    return matchesSearch && matchesDiscipline;
  });

  return (
    <div className="min-h-screen bg-gray-50 text-gray-900 font-sans">
      {/* --- Header Section --- */}
      <header className="sticky top-0 z-10 bg-white border-b border-gray-200 px-6 py-4 shadow-sm">
        <div className="max-w-7xl mx-auto flex flex-col md:flex-row md:items-center justify-between gap-4">
          <h1 className="text-2xl font-bold tracking-tight text-blue-600">Anti-FOMO</h1>
          
          <div className="flex flex-1 max-w-2xl gap-2">
            <input 
              type="text" 
              placeholder="Search articles, jobs, events..." 
              className="w-full px-4 py-2 bg-gray-100 border border-transparent rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:bg-white transition-all"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>

          <div className="flex gap-2 overflow-x-auto pb-1 md:pb-0 scrollbar-hide">
            {DISCIPLINES.map(discipline => (
              <button
                key={discipline}
                onClick={() => setSelectedDiscipline(discipline)}
                className={`px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
                  selectedDiscipline === discipline 
                    ? 'bg-blue-600 text-white' 
                    : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
                }`}
              >
                {discipline}
              </button>
            ))}
          </div>
        </div>
      </header>

      {/* --- Main Content --- */}
      <main className="max-w-7xl mx-auto px-6 py-8">
        <div className="mb-6 flex justify-between items-center">
          <h2 className="text-lg font-semibold text-gray-700">
            {loading ? 'Fetching latest data...' : `${filteredItems.length} Results ${selectedDiscipline !== 'All' ? `for ${selectedDiscipline}` : ''}`}
          </h2>
          {loading && (
            <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-blue-600"></div>
          )}
        </div>

        {error && (
          <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded-lg mb-6 flex items-center gap-3">
            <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
              <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
            </svg>
            <p className="text-sm font-medium">{error}</p>
          </div>
        )}

        {/* --- Card Grid --- */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {!loading && filteredItems.map((item, index) => (
            <div 
              key={index} 
              className="group bg-white rounded-xl border border-gray-200 overflow-hidden hover:shadow-lg hover:border-blue-200 transition-all duration-300 flex flex-col"
            >
              <div className="p-6 flex flex-col flex-1">
                <div className="flex items-center justify-between mb-3">
                  <span className={`text-xs font-bold uppercase tracking-wider px-2 py-1 rounded ${
                    item.item_type === 'Internship' || item.item_type === 'Job' 
                      ? 'bg-green-100 text-green-700' 
                      : item.item_type === 'Event' 
                        ? 'bg-purple-100 text-purple-700' 
                        : 'bg-blue-100 text-blue-700'
                  }`}>
                    {item.item_type}
                  </span>
                  <span className="text-xs text-gray-400 font-medium">{item.source_platform}</span>
                </div>
                
                <h3 className="text-lg font-bold mb-2 group-hover:text-blue-600 transition-colors leading-snug">
                  {item.title}
                </h3>
                
                <p className="text-gray-600 text-sm mb-4 line-clamp-3">
                  {item.content_text || "No description available."}
                </p>

                <div className="flex items-center justify-between mt-auto pt-4 border-t border-gray-50">
                  <span className="text-xs font-medium text-gray-500 bg-gray-50 px-2 py-1 rounded">
                    {item.discipline}
                  </span>
                  <a 
                    href={item.url} 
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm font-semibold text-blue-600 hover:text-blue-800 flex items-center gap-1"
                  >
                    View
                    <svg xmlns="http://www.w3.org/2000/svg" className="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                      <path fillRule="evenodd" d="M10.293 3.293a1 1 0 011.414 0l6 6a1 1 0 010 1.414l-6 6a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-4.293-4.293a1 1 0 010-1.414z" clipRule="evenodd" />
                    </svg>
                  </a>
                </div>
              </div>
            </div>
          ))}
        </div>

        {loading && (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
            {[1, 2, 3, 4, 5, 6].map(n => (
              <div key={n} className="bg-white rounded-xl border border-gray-100 p-6 animate-pulse">
                <div className="flex justify-between mb-4">
                  <div className="h-4 w-16 bg-gray-200 rounded"></div>
                  <div className="h-4 w-24 bg-gray-100 rounded"></div>
                </div>
                <div className="h-6 w-full bg-gray-200 rounded mb-3"></div>
                <div className="h-4 w-3/4 bg-gray-100 rounded mb-6"></div>
                <div className="flex justify-between">
                  <div className="h-4 w-20 bg-gray-50 rounded"></div>
                  <div className="h-4 w-12 bg-gray-100 rounded"></div>
                </div>
              </div>
            ))}
          </div>
        )}

        {!loading && filteredItems.length === 0 && (
          <div className="text-center py-20">
            <div className="text-gray-400 mb-4">
              <svg xmlns="http://www.w3.org/2000/svg" className="h-16 w-16 mx-auto" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9.172 9.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <h3 className="text-xl font-semibold text-gray-600">No items found</h3>
            <p className="text-gray-500">Try adjusting your search or filters.</p>
          </div>
        )}
      </main>
    </div>
  );
}
