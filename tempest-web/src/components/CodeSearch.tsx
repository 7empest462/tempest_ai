import { useState } from 'react';
import { useStore } from '../store';
import { Search, Loader2, FileCode } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

export function CodeSearch() {
  const [query, setQuery] = useState('');
  const { searchResults, isSearching, setSearching, setSearchResults } = useStore();

  const handleSearch = () => {
    if (!query.trim() || isSearching) return;
    setSearching(true);
    setSearchResults([]);
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('SearchFiles', { query, path: '.' });
    }
  };

  const handleResultClick = (file: string) => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('ReadFile', { path: file });
    }
  };

  return (
    <div className="flex flex-col h-full">
      <div className="flex gap-2 p-2 mb-3 bg-white/5 rounded-lg border border-white/10">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
          className="flex-1 bg-transparent px-2 py-1 text-sm focus:outline-none text-white placeholder-muted-foreground"
          placeholder="Search regex / string..."
          disabled={isSearching}
        />
        <button
          onClick={handleSearch}
          disabled={isSearching || !query.trim()}
          className="p-2 bg-accent text-background rounded hover:bg-accent/90 transition-colors disabled:opacity-50 flex items-center justify-center"
        >
          {isSearching ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {isSearching && (
          <div className="flex flex-col items-center justify-center p-6 text-muted-foreground text-sm font-mono gap-2">
            <Loader2 className="animate-spin text-accent" size={20} />
            <span>Scanning project tree...</span>
          </div>
        )}

        {!isSearching && searchResults.length === 0 && query && (
          <p className="text-muted-foreground text-sm p-4 text-center italic">No matches found</p>
        )}

        <div className="flex flex-col gap-3">
          <AnimatePresence>
            {!isSearching && searchResults.map((match: any, idx: number) => (
              <motion.div
                key={`${match.file}-${match.line}-${idx}`}
                onClick={() => handleResultClick(match.file)}
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: Math.min(idx * 0.02, 0.3) }}
                className="p-3 bg-white/5 border border-white/5 hover:border-accent/40 rounded-lg cursor-pointer transition-all hover:bg-accent/5"
              >
                <div className="flex items-center gap-2 mb-1 text-xs font-mono text-accent truncate">
                  <FileCode size={12} />
                  <span className="truncate">{match.file}</span>
                  <span className="text-muted-foreground ml-auto">Line {match.line}</span>
                </div>
                <pre className="text-xs font-mono text-muted-foreground truncate bg-black/30 p-1.5 rounded border border-white/5">
                  {match.content}
                </pre>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
}
