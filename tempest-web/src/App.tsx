import { useEffect, useRef } from 'react';
// @ts-ignore
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle } from 'react-resizable-panels';
import type { PanelImperativeHandle } from 'react-resizable-panels';
import { useStore } from './store';
import { TerminalPanel } from './components/TerminalPanel';
import { FileExplorer } from './components/FileExplorer';
import { ChatInterface } from './components/ChatInterface';
import { MissionControl } from './components/MissionControl';
import { CodeEditor } from './components/CodeEditor';
import { CommandPalette } from './components/CommandPalette';
import { CodeSearch } from './components/CodeSearch';
import { SettingsPanel } from './components/SettingsPanel';
import { SafeModeModal } from './components/SafeModeModal';
import { AskUserModal } from './components/AskUserModal';
import { AgentTimeline } from './components/AgentTimeline';
import { TurnReviewModal } from './components/TurnReviewModal';
import { useNexusSocket } from './hooks/useNexusSocket';
import * as wasm from './pkg/tempest_wasm.js';
import { Folder, Brain, Search, Settings, Terminal } from 'lucide-react';
import { playTabSwitchSound, playPanelResizeSound } from './utils/audio';

export default function App() {
  const sidebarRef = useRef<PanelImperativeHandle>(null);
  const {
    isConnected,
    engineStatus,
    cpu,
    gpu,
    ram,
    tps,
    ctxUsed,
    ctxTotal,
    backgroundIntensity,
    isEditorFocused,
    activeFile,
    setActiveFile,
    isTerminalOpen,
    setTerminalOpen,
    activeTab,
    setActiveTab,
    chatViewMode,
    setChatViewMode,
    isFileEditable,
    setFileEditable,
  } = useStore();

  useNexusSocket();

  useEffect(() => {
    const timer = setTimeout(() => {
      if (sidebarRef.current) {
        const size = sidebarRef.current.getSize();
        if (size.asPercentage < 10) {
          sidebarRef.current.resize(activeFile ? '20' : '25');
        }
      }
    }, 100);
    return () => clearTimeout(timer);
  }, [activeFile]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'j') {
        e.preventDefault();
        setTerminalOpen(!isTerminalOpen);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isTerminalOpen, setTerminalOpen]);

  const initWasm = async () => {
    try {
      const dashboard = await wasm.initialize_dashboard('vortex-canvas');
      console.log('🌪️ WASM Dashboard Online');

      const resizeFn = () => {
        const canvas = document.getElementById('vortex-canvas') as HTMLCanvasElement;
        if (canvas) {
          canvas.width = window.innerWidth;
          canvas.height = window.innerHeight;
          dashboard.resize(window.innerWidth, window.innerHeight);
        }
      };

      window.addEventListener('resize', resizeFn);
    } catch (e) {
      console.error('❌ Failed to load WASM:', e);
    }
  };

  useEffect(() => {
    initWasm();
  }, []);

  const bgOpacity =
    backgroundIntensity === 'subtle'
      ? 'opacity-20'
      : backgroundIntensity === 'medium'
        ? 'opacity-50'
        : 'opacity-100';

  const vortexClasses = `fixed inset-0 z-[-1] transition-all duration-700 pointer-events-none ${bgOpacity} ${isEditorFocused ? 'blur-[8px] opacity-10 scale-105' : ''}`;

  return (
    <div className="h-screen w-screen overflow-hidden flex flex-col relative text-foreground">
      {/* Modals & Overlays */}
      <SafeModeModal />
      <AskUserModal />
      <TurnReviewModal />
      <CommandPalette />

      {/* Background WASM Canvas */}
      <div className={vortexClasses}>
        <canvas id="vortex-canvas" className="w-full h-full block" />
      </div>

      {/* Header */}
      <header className="flex-none h-14 glass-panel border-b border-border/50 flex items-center justify-between px-6 z-10">
        <div className="flex items-center gap-3">
          <span className="text-xl">🌪️</span>
          <h1 className="text-lg font-semibold tracking-widest">
            TEMPEST{' '}
            <span className="text-accent drop-shadow-[0_0_8px_rgba(0,242,255,0.4)]">AI</span>
          </h1>
        </div>
        <div className="flex items-center gap-6">
          <button
            onClick={() => setChatViewMode(chatViewMode === 'classic' ? 'timeline' : 'classic')}
            className="flex items-center gap-2 px-3 py-1.5 rounded-md border text-xs font-mono transition-all cursor-pointer bg-white/5 border-border text-muted-foreground hover:bg-white/10 hover:text-white"
          >
            {chatViewMode === 'classic' ? 'CLASSIC VIEW' : 'TIMELINE VIEW'}
          </button>

          <button
            onClick={() => setTerminalOpen(!isTerminalOpen)}
            className={`flex items-center gap-2 px-3 py-1.5 rounded-md border text-xs font-mono transition-all cursor-pointer ${
              isTerminalOpen
                ? 'bg-accent/15 border-accent text-accent shadow-[0_0_10px_rgba(0,242,255,0.15)]'
                : 'bg-white/5 border-border text-muted-foreground hover:bg-white/10 hover:text-white'
            }`}
          >
            <Terminal size={14} /> TERMINAL
          </button>

          <div className="flex gap-4 font-mono text-xs text-muted-foreground">
            <span className="bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent">
              CPU: {cpu.toFixed(1)}%
            </span>
            <span className="bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent">
              GPU: {gpu.toFixed(1)}%
            </span>
            <span className="bg-white/5 px-3 py-1 rounded-md border-l-2 border-accent">
              RAM: {ram}
            </span>
          </div>
        </div>
      </header>

      {/* Main Content */}
      <main className="flex-1 min-h-0 relative z-10 flex p-4 pb-0">
        <PanelGroup
          key={activeFile ? 'workspace-3' : 'workspace-2'}
          orientation="horizontal"
          className="h-full w-full"
          id="workspace-group"
          // @ts-ignore
          onLayout={() => playPanelResizeSound()}
        >
          {/* Left Panel: Sidebar */}
          <Panel
            id="sidebar-panel"
            panelRef={sidebarRef}
            defaultSize={activeFile ? '20' : '25'}
            minSize="15"
            maxSize="35"
            style={{ minWidth: '240px', maxWidth: '400px' }}
            className="glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col"
          >
            {/* Sidebar Tab Header */}
            <div className="flex border-b border-border/50 bg-black/20 p-1 gap-1">
              {[
                { id: 'files', label: 'Explorer', icon: Folder },
                { id: 'agent', label: 'Core', icon: Brain },
                { id: 'search', label: 'Diagnostics', icon: Search },
                { id: 'settings', label: 'Tuning', icon: Settings },
              ].map((tab) => {
                const Icon = tab.icon;
                const active = activeTab === tab.id;
                return (
                  <button
                    key={tab.id}
                    onClick={() => {
                      playTabSwitchSound();
                      setActiveTab(tab.id as any);
                    }}
                    className={`flex-1 py-2 rounded-lg flex flex-col items-center gap-1 text-[10px] uppercase font-bold tracking-wider transition-all cursor-pointer ${
                      active
                        ? 'bg-white/10 text-white border border-white/5 shadow-sm'
                        : 'text-muted-foreground hover:text-white hover:bg-white/5 border border-transparent'
                    }`}
                  >
                    <Icon size={14} className={active ? 'text-accent' : ''} />
                    <span>{tab.label}</span>
                  </button>
                );
              })}
            </div>

            {/* Sidebar Content Area */}
            <div className="flex-1 p-4 overflow-y-auto min-h-0">
              {activeTab === 'files' && <FileExplorer />}
              {activeTab === 'agent' && <MissionControl />}
              {activeTab === 'search' && <CodeSearch />}
              {activeTab === 'settings' && <SettingsPanel />}
            </div>
          </Panel>

          <PanelResizeHandle className="w-1.5 hover:bg-accent/30 transition-all cursor-col-resize relative flex items-center justify-center bg-white/5 border-l border-r border-white/5 duration-150">
            <div className="w-0.5 h-8 bg-muted-foreground/20 rounded-full" />
          </PanelResizeHandle>

          {/* Center Panel: Chat */}
          <Panel
            id="chat-panel"
            defaultSize={activeFile ? '40' : '75'}
            minSize="30"
            style={{ minWidth: '320px' }}
            className="glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col"
          >
            {chatViewMode === 'classic' ? <ChatInterface /> : <AgentTimeline />}
          </Panel>

          {/* Right Panel: Editor (conditionally shown) */}
          {activeFile && (
            <>
              <PanelResizeHandle className="w-1.5 hover:bg-accent/30 transition-all cursor-col-resize relative flex items-center justify-center bg-white/5 border-l border-r border-white/5 duration-150">
                <div className="w-0.5 h-8 bg-muted-foreground/20 rounded-full" />
              </PanelResizeHandle>
              <Panel
                id="editor-panel"
                defaultSize="40"
                minSize="20"
                style={{ minWidth: '300px' }}
                className="glass-panel border border-border/50 rounded-xl overflow-hidden flex flex-col"
              >
                <div className="p-3 border-b border-border/50 flex justify-between items-center bg-black/20">
                  <span className="font-mono text-sm truncate pr-4">{activeFile.name}</span>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => {
                        if (isFileEditable) {
                          // Save changes
                          // @ts-ignore
                          if (window.sendNexus && activeFile) {
                            // @ts-ignore
                            window.sendNexus('WriteFile', {
                              path: activeFile.name,
                              content: activeFile.content,
                            });
                          }
                        }
                        setFileEditable(!isFileEditable);
                      }}
                      className={`text-xs font-semibold px-3 py-1 rounded transition-colors cursor-pointer ${isFileEditable ? 'bg-green-500/20 text-green-400 hover:bg-green-500/30' : 'bg-white/10 hover:bg-white/20'}`}
                    >
                      {isFileEditable ? 'SAVE' : 'EDIT'}
                    </button>
                    <button
                      onClick={() => setActiveFile(null)}
                      className="text-muted-foreground hover:text-white px-2 py-1 hover:bg-white/5 rounded transition-all cursor-pointer text-sm font-bold"
                      title="Close File"
                    >
                      ✕
                    </button>
                  </div>
                </div>
                <CodeEditor />
              </Panel>
            </>
          )}
        </PanelGroup>
      </main>

      {/* Bottom Terminal Overlay Panel */}
      <div
        className={`absolute bottom-4 left-1/2 -translate-x-1/2 w-[80%] h-64 glass-panel border border-border/50 rounded-t-xl z-30 transition-all duration-300 ${isTerminalOpen ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0 pointer-events-none'}`}
      >
        <div className="flex items-center justify-between p-2 border-b border-border/50 bg-black/40">
          <span className="font-mono text-xs pl-2">🖥️ TERMINAL</span>
          <button
            onClick={() => setTerminalOpen(false)}
            className="text-muted-foreground hover:text-white px-2"
          >
            ✕
          </button>
        </div>
        <div className="h-[calc(100%-40px)] w-full">{isTerminalOpen && <TerminalPanel />}</div>
      </div>

      {/* Footer */}
      <footer className="flex-none h-10 glass-panel border-t border-border/50 flex items-center justify-between px-6 z-10 text-xs font-mono text-muted-foreground mt-4">
        <div className="flex items-center gap-3">
          <span
            className={`w-2 h-2 rounded-full ${isConnected ? 'bg-green-500 shadow-[0_0_8px_#00ff88]' : 'bg-red-500 shadow-[0_0_8px_#ff0000]'}`}
          />
          <span>Engine: {engineStatus}</span>
        </div>
        <div className="flex items-center gap-4">
          <span>TPS: {tps}</span>
          <span>│</span>
          <span>
            CTX: {ctxUsed >= 1024 ? `${(ctxUsed / 1024).toFixed(1)}k` : ctxUsed}/
            {ctxTotal >= 1024 ? `${(ctxTotal / 1024).toFixed(0)}k` : ctxTotal}
          </span>
        </div>
      </footer>
    </div>
  );
}
