import { useState } from 'react';
import { useStore } from '../store';
import {
  Search,
  Loader2,
  FileCode,
  Cpu,
  ChevronDown,
  Brain,
  Wrench,
  ShieldCheck,
  Database,
  Clock,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { playTabSwitchSound } from '../utils/audio';

export function CodeSearch() {
  const [query, setQuery] = useState('');
  const [modelsOpen, setModelsOpen] = useState(true);
  const {
    searchResults,
    isSearching,
    setSearching,
    setSearchResults,
    engineStatus,
    plannerModel,
    executorModel,
    verifierModel,
    kvCacheHitPct,
    planningDurationMs,
    executingDurationMs,
    verifyingDurationMs,
  } = useStore();

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
    <div className="flex flex-col h-full gap-4">
      {/* Active Models (Collapsible) */}
      <div className="bg-white/[0.01] border border-white/5 rounded-xl flex flex-col flex-none hover:border-white/10 transition-colors overflow-hidden">
        <div
          onClick={() => setModelsOpen(!modelsOpen)}
          className="p-3 flex items-center justify-between cursor-pointer select-none"
        >
          <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
            <Cpu size={12} className="text-accent" /> Active Models &amp; Engine
          </h4>
          <motion.div
            animate={{ rotate: modelsOpen ? 180 : 0 }}
            transition={{ duration: 0.15 }}
            className="text-muted-foreground"
          >
            <ChevronDown size={14} />
          </motion.div>
        </div>

        <div
          className={`grid transition-all duration-200 ease-in-out ${
            modelsOpen ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'
          }`}
        >
          <div className="overflow-hidden">
            <div className="px-3 pb-3">
              {/* Detect VRAM Sharing: all 3 models identical and non-placeholder */}
              {plannerModel === executorModel &&
              executorModel === verifierModel &&
              plannerModel !== '--' ? (
                /* Unified VRAM Sharing Layout */
                <div className="flex flex-col gap-2 text-[10px] font-mono">
                  <div className="bg-black/35 border border-white/5 px-3 py-2.5 rounded-lg flex flex-col gap-1.5 select-text hover:bg-black/45 transition-colors">
                    <div className="flex items-center justify-between">
                      <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold">
                        Engine
                      </span>
                      <span className="text-[7px] font-bold uppercase tracking-widest bg-accent/15 text-accent border border-accent/25 px-2 py-0.5 rounded-full">
                        VRAM Sharing
                      </span>
                    </div>
                    <span
                      className="text-accent truncate font-bold text-[11px]"
                      title={engineStatus}
                    >
                      {engineStatus.replace(' (VRAM Sharing)', '')}
                    </span>
                  </div>
                  <div className="bg-black/35 border border-white/5 px-3 py-2.5 rounded-lg flex flex-col gap-1.5 select-text hover:bg-black/45 transition-colors">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1">
                      <Brain size={8} className="text-purple-400" /> Unified Model
                    </span>
                    <span
                      className="text-purple-400 truncate font-bold text-[11px]"
                      title={plannerModel}
                    >
                      {plannerModel}
                    </span>
                    <span className="text-[8px] text-muted-foreground/50 leading-tight">
                      Planner · Executor · Verifier — single model, dynamic system prompt switching
                    </span>
                  </div>
                </div>
              ) : (
                /* Standard 3-Tier Layout */
                <div className="grid grid-cols-2 gap-2 text-[10px] font-mono">
                  <div className="bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold">
                      Engine
                    </span>
                    <span className="text-accent truncate font-bold" title={engineStatus}>
                      {engineStatus}
                    </span>
                  </div>
                  <div className="bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1">
                      <Brain size={8} className="text-purple-400" /> Planner
                    </span>
                    <span className="text-purple-400 truncate font-bold" title={plannerModel}>
                      {plannerModel}
                    </span>
                  </div>
                  <div className="bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1">
                      <Wrench size={8} className="text-pink-400" /> Executor
                    </span>
                    <span className="text-pink-400 truncate font-bold" title={executorModel}>
                      {executorModel}
                    </span>
                  </div>
                  <div className="bg-black/35 border border-white/5 px-2.5 py-1.5 rounded-lg flex flex-col gap-0.5 select-text hover:bg-black/45 transition-colors">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1">
                      <ShieldCheck size={8} className="text-emerald-400" /> Verifier
                    </span>
                    <span className="text-emerald-400 truncate font-bold" title={verifierModel}>
                      {verifierModel}
                    </span>
                  </div>
                </div>
              )}

              {/* KV Cache Hit Rate Progress Bar */}
              {kvCacheHitPct !== null && (
                <div className="mt-3 pt-3 border-t border-white/5 flex flex-col gap-2 font-mono text-[10px]">
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1.5">
                      <Database size={10} className="text-[#00f2ff]" /> KV Cache Hit Rate
                    </span>
                    <span className="text-[#00f2ff] font-bold font-mono">
                      {kvCacheHitPct.toFixed(1)}%
                    </span>
                  </div>
                  <div className="w-full bg-black/40 border border-white/5 h-2 rounded-full overflow-hidden relative">
                    <motion.div
                      initial={{ width: 0 }}
                      animate={{ width: `${kvCacheHitPct}%` }}
                      transition={{ duration: 0.8, ease: 'easeOut' }}
                      className="bg-gradient-to-r from-purple-500 to-[#00f2ff] h-full rounded-full"
                    />
                  </div>
                </div>
              )}

              {/* Turn Phase Durations */}
              {(planningDurationMs !== null ||
                executingDurationMs !== null ||
                verifyingDurationMs !== null) && (
                <div className="mt-3 pt-3 border-t border-white/5 flex flex-col gap-2 font-mono text-[10px]">
                  <span className="text-muted-foreground/60 uppercase text-[8px] tracking-wider font-semibold flex items-center gap-1.5 mb-1">
                    <Clock size={10} className="text-cyan-400" /> Turn Phase Durations
                  </span>

                  {/* Planning Duration */}
                  {planningDurationMs !== null && (
                    <div className="flex flex-col gap-1">
                      <div className="flex items-center justify-between text-[9px]">
                        <span className="text-purple-400 font-semibold">Planning</span>
                        <span className="text-purple-300 font-bold">{planningDurationMs} ms</span>
                      </div>
                      <div className="w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5">
                        <motion.div
                          initial={{ width: 0 }}
                          animate={{
                            width: `${Math.min(100, (planningDurationMs / 10000) * 100)}%`,
                          }}
                          transition={{ duration: 0.8, ease: 'easeOut' }}
                          className="bg-purple-500 h-full rounded-full"
                        />
                      </div>
                    </div>
                  )}

                  {/* Execution Duration */}
                  {executingDurationMs !== null && (
                    <div className="flex flex-col gap-1">
                      <div className="flex items-center justify-between text-[9px]">
                        <span className="text-pink-400 font-semibold">Execution (Tool Use)</span>
                        <span className="text-pink-300 font-bold">{executingDurationMs} ms</span>
                      </div>
                      <div className="w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5">
                        <motion.div
                          initial={{ width: 0 }}
                          animate={{
                            width: `${Math.min(100, (executingDurationMs / 15000) * 100)}%`,
                          }}
                          transition={{ duration: 0.8, ease: 'easeOut' }}
                          className="bg-pink-500 h-full rounded-full"
                        />
                      </div>
                    </div>
                  )}

                  {/* Verification Duration */}
                  {verifyingDurationMs !== null && (
                    <div className="flex flex-col gap-1">
                      <div className="flex items-center justify-between text-[9px]">
                        <span className="text-emerald-400 font-semibold">Verification</span>
                        <span className="text-emerald-300 font-bold">{verifyingDurationMs} ms</span>
                      </div>
                      <div className="w-full bg-black/40 h-1.5 rounded-full overflow-hidden relative border border-white/5">
                        <motion.div
                          initial={{ width: 0 }}
                          animate={{
                            width: `${Math.min(100, (verifyingDurationMs / 10000) * 100)}%`,
                          }}
                          transition={{ duration: 0.8, ease: 'easeOut' }}
                          className="bg-emerald-500 h-full rounded-full"
                        />
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="bg-white/[0.01] border border-white/5 rounded-xl flex flex-col flex-none hover:border-white/10 transition-colors p-3 mb-1">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5 mb-2">
          <Brain size={12} className="text-accent" /> Quick Actions
        </h4>
        <div className="grid grid-cols-2 gap-2">
          {[
            {
              label: 'Generate tests for current file',
              query: 'Generate tests for the current file.',
            },
            {
              label: 'Optimize function speed',
              query: 'Optimize the current function or file for maximum performance and speed.',
            },
            {
              label: 'Add error handling',
              query: 'Add robust error handling everywhere in the current file.',
            },
            {
              label: "Explain like I'm 10",
              query: 'Explain this codebase to me like I am 10 years old.',
            },
            {
              label: 'Audit for security issues',
              query:
                'Audit the current file for potential security vulnerabilities and edge cases.',
            },
            {
              label: 'Refactor for readability',
              query:
                'Refactor the code to improve readability, variable naming, and maintainability.',
            },
            {
              label: 'Generate documentation',
              query:
                'Generate comprehensive docstrings and comments for all public functions and types.',
            },
            {
              label: 'Find hidden bugs & leaks',
              query:
                'Analyze the current file for memory leaks, resource exhaustion, and subtle hidden bugs.',
            },
          ].map((action, i) => (
            <button
              key={i}
              onClick={() => {
                playTabSwitchSound();
                // @ts-ignore
                if (window.sendNexus) {
                  // @ts-ignore
                  window.sendNexus('Chat', { message: action.query });
                }
              }}
              className="text-[10px] font-semibold text-left p-2 rounded bg-white/5 hover:bg-accent/20 hover:text-accent border border-white/5 hover:border-accent/30 transition-all text-muted-foreground cursor-pointer"
            >
              {action.label}
            </button>
          ))}
        </div>
      </div>

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
            {!isSearching &&
              searchResults.map((match: any, idx: number) => (
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
