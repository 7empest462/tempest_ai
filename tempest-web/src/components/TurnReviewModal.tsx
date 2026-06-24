import { useState } from 'react';
import { useStore } from '../store';
import { Check, X, Code, FileText, ChevronRight, AlertTriangle } from 'lucide-react';
import { motion } from 'framer-motion';
import { DiffEditor } from '@monaco-editor/react';

const getLanguageFromPath = (path: string): string => {
  const ext = path.split('.').pop()?.toLowerCase() || '';
  const extMap: Record<string, string> = {
    rs: 'rust',
    zig: 'zig',
    ts: 'typescript',
    tsx: 'typescript',
    js: 'javascript',
    jsx: 'javascript',
    sh: 'shell',
    bash: 'shell',
    nix: 'nix',
    toml: 'toml',
    lock: 'toml',
    md: 'markdown',
    json: 'json',
    html: 'html',
    css: 'css',
    py: 'python',
    yml: 'yaml',
    yaml: 'yaml',
    c: 'c',
    cpp: 'cpp',
    h: 'cpp',
    txt: 'plaintext',
  };
  return extMap[ext] || 'plaintext';
};

export function TurnReviewModal() {
  const { turnReviewRequest, setTurnReviewRequest } = useStore();
  const [selectedFileIndex, setSelectedFileIndex] = useState(0);

  if (!turnReviewRequest || turnReviewRequest.files.length === 0) return null;

  const currentFile = turnReviewRequest.files[selectedFileIndex] || turnReviewRequest.files[0];

  const handleAcceptAll = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('ReviewApprove', {});
    }
    setTurnReviewRequest(null);
  };

  const handleRejectAll = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('ReviewReject', {});
    }
    setTurnReviewRequest(null);
  };

  const handleTweak = () => {
    if (!currentFile) return;
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('ReadFile', { path: currentFile.path });
      // Make file editable
      useStore.getState().setFileEditable(true);
      // Focus the editor
      useStore.getState().setEditorFocused(true);
      // Switch active tab to 'files' or open editor view
    }
    setTurnReviewRequest(null);
  };

  return (
    <div className="fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/70 backdrop-blur-md">
      <motion.div
        initial={{ opacity: 0, scale: 0.97 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.97 }}
        className="w-[1250px] max-w-full h-[85vh] glass-panel border border-border/80 rounded-2xl shadow-[0_0_60px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40 flex-none">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-accent/10 border border-accent/30 flex items-center justify-center text-accent">
              <Code size={20} className="drop-shadow-[0_0_6px_rgba(0,242,255,0.4)]" />
            </div>
            <div>
              <h3 className="font-semibold text-sm tracking-wider text-white">
                TURN COMPLETION REVIEW
              </h3>
              <p className="text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5">
                Verify and commit code changes from the agent's turn
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 bg-amber-500/10 border border-amber-500/20 px-3 py-1.5 rounded-lg text-amber-400 font-mono text-[10px] uppercase">
            <AlertTriangle size={12} />
            <span>Workspace Changes Pending Approval</span>
          </div>
        </div>

        {/* Layout Body */}
        <div className="flex-1 min-h-0 flex flex-row">
          {/* Left Sidebar: Modified Files list */}
          <div className="w-[320px] flex-shrink-0 border-r border-border/50 bg-black/25 flex flex-col">
            <div className="p-4 border-b border-border/30 bg-white/[0.01]">
              <span className="text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                Modified Files ({turnReviewRequest.files.length})
              </span>
            </div>
            <div className="flex-1 overflow-y-auto p-2 flex flex-col gap-1">
              {turnReviewRequest.files.map((file, idx) => {
                const fileName = file.path.split('/').pop() || file.path;
                const fileDir = file.path.split('/').slice(0, -1).join('/');
                const isSelected = selectedFileIndex === idx;

                return (
                  <button
                    key={idx}
                    onClick={() => setSelectedFileIndex(idx)}
                    className={`w-full text-left p-3 rounded-xl border transition-all duration-200 flex items-start gap-3 cursor-pointer ${
                      isSelected
                        ? 'bg-accent/10 border-accent/40 text-white'
                        : 'bg-transparent border-transparent hover:bg-white/5 text-muted-foreground hover:text-white'
                    }`}
                  >
                    <FileText
                      size={16}
                      className={`mt-0.5 ${isSelected ? 'text-accent' : 'text-muted-foreground/60'}`}
                    />
                    <div className="flex-1 min-w-0 flex flex-col gap-0.5">
                      <span className="text-xs font-bold font-mono truncate">{fileName}</span>
                      {fileDir && (
                        <span className="text-[9px] font-mono opacity-50 truncate">{fileDir}</span>
                      )}
                    </div>
                    <ChevronRight
                      size={14}
                      className={`mt-1 transition-transform ${isSelected ? 'text-accent translate-x-0.5' : 'opacity-0'}`}
                    />
                  </button>
                );
              })}
            </div>
          </div>

          {/* Right Diff Editor Panel */}
          <div className="flex-1 min-w-0 flex flex-col bg-black/10">
            <div className="px-6 py-3 border-b border-border/30 bg-white/[0.01] flex items-center justify-between">
              <span className="text-[10px] font-bold font-mono text-muted-foreground">
                PATH: <span className="text-white">{currentFile?.path}</span>
              </span>
              <span className="text-[9px] font-mono bg-purple-500/10 text-purple-400 border border-purple-500/20 px-2 py-0.5 rounded uppercase">
                {getLanguageFromPath(currentFile?.path || '')}
              </span>
            </div>

            <div className="flex-1 min-h-0 select-text">
              {currentFile ? (
                <DiffEditor
                  height="100%"
                  language={getLanguageFromPath(currentFile.path)}
                  theme="vs-dark"
                  original={currentFile.original}
                  modified={currentFile.modified}
                  options={{
                    readOnly: true,
                    minimap: { enabled: false },
                    fontSize: 12,
                    fontFamily: '"JetBrains Mono", monospace',
                    scrollBeyondLastLine: false,
                    renderSideBySide: true,
                    smoothScrolling: true,
                  }}
                  loading={
                    <div className="flex items-center justify-center h-full text-accent font-mono text-xs animate-pulse">
                      Generating side-by-side comparison...
                    </div>
                  }
                />
              ) : (
                <div className="flex items-center justify-center h-full text-muted-foreground text-xs font-mono">
                  Select a file from the sidebar to inspect diff details.
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Footer actions */}
        <div className="px-6 py-4 border-t border-border/50 bg-black/40 flex items-center justify-between flex-none">
          <span className="text-[10px] text-muted-foreground/60 font-mono">
            Pro Tip: Click 'Tweak' to close review and make manual edits in the main panel.
          </span>
          <div className="flex items-center gap-3">
            <button
              onClick={handleTweak}
              className="flex items-center gap-2 px-4 py-2.5 rounded-lg border border-white/10 hover:border-white/20 bg-white/5 text-white text-xs font-bold uppercase tracking-wider hover:bg-white/10 active:translate-y-px transition-all cursor-pointer"
            >
              <Code size={14} /> Tweak Code
            </button>
            <button
              onClick={handleRejectAll}
              className="flex items-center gap-2 px-4 py-2.5 rounded-lg border border-red-500/30 bg-red-500/10 text-red-400 text-xs font-bold uppercase tracking-wider hover:bg-red-500/20 active:translate-y-px transition-all cursor-pointer"
            >
              <X size={14} /> Reject All
            </button>
            <button
              onClick={handleAcceptAll}
              className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] cursor-pointer"
            >
              <Check size={14} /> Accept & Commit
            </button>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
