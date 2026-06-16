import { useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useStore } from '../store';
import { Send, Square, GitCommitHorizontal, CheckCircle2, CircleDashed, Cpu, TerminalSquare, AlertCircle, RotateCcw } from 'lucide-react';

function CollapsibleThought({ reasoning, title = "Thought Process", defaultOpen = false }: { reasoning: string; title?: string; defaultOpen?: boolean }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  return (
    <div className="text-xs text-muted-foreground border-l-2 border-accent/50 pl-3 py-1">
      <div 
        onClick={() => setIsOpen(!isOpen)}
        className="cursor-pointer font-semibold select-none hover:text-white transition-colors flex items-center gap-2 w-max"
      >
        <GitCommitHorizontal size={14} className="text-accent" />
        <span>{title}</span>
        <motion.span 
          animate={{ rotate: isOpen ? 90 : 0 }}
          transition={{ duration: 0.15 }}
          className="text-[10px] opacity-60 inline-block"
        >
          ▶
        </motion.span>
      </div>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0, marginTop: 0 }}
            animate={{ height: 'auto', opacity: 1, marginTop: 8 }}
            exit={{ height: 0, opacity: 0, marginTop: 0 }}
            transition={{ duration: 0.2, ease: 'easeInOut' }}
            className="overflow-hidden"
          >
            <div className="font-mono whitespace-pre-wrap opacity-70 bg-black/20 p-3 rounded">
              {reasoning}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function CollapsibleTool({ tool, defaultOpen = false }: { tool: { name: string; args?: string; output?: string; success: boolean }; defaultOpen?: boolean }) {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  return (
    <div className="bg-black/20 border border-white/5 rounded block overflow-hidden animate-in fade-in duration-300">
      <div 
        onClick={() => setIsOpen(!isOpen)}
        className="text-xs cursor-pointer select-none py-2 px-3 text-purple-400 font-semibold hover:bg-white/5 transition-colors flex items-center gap-2"
      >
        <TerminalSquare size={14} />
        <span>Executed: {tool.name}</span>
        <motion.span 
          animate={{ rotate: isOpen ? 90 : 0 }}
          transition={{ duration: 0.15 }}
          className="text-[10px] opacity-60 inline-block"
        >
          ▶
        </motion.span>
        {tool.success ? (
          <CheckCircle2 size={12} className="text-green-500 ml-auto" />
        ) : (
          <AlertCircle size={12} className="text-red-500 ml-auto" />
        )}
      </div>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: 'easeInOut' }}
            className="overflow-hidden"
          >
            <div className="p-3 border-t border-white/5 text-[11px] font-mono">
              {tool.args && (
                <div className="mb-2">
                  <strong className="text-muted-foreground">Input:</strong>
                  <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto bg-black/40 p-2 mt-1 rounded">
                    {tool.args}
                  </pre>
                </div>
              )}
              {tool.output && (
                <div>
                  <strong className="text-muted-foreground">Output:</strong>
                  <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto max-h-60 bg-black/40 p-2 mt-1 rounded">
                    {tool.output}
                  </pre>
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ParallelToolsDashboard() {
  const { activeToolExecutions } = useStore();
  if (!activeToolExecutions || activeToolExecutions.length === 0) return null;

  return (
    <div className="flex flex-col gap-3 my-4 p-4 rounded-xl border border-white/5 bg-white/[0.02] backdrop-blur-sm animate-in fade-in slide-in-from-bottom-2 duration-300">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-bold text-accent uppercase tracking-wider flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-accent animate-ping" />
          Parallel Execution Streams ({activeToolExecutions.filter(e => e.status === 'running').length} Active)
        </h4>
        <div className="text-[10px] text-muted-foreground font-mono">
          Sync: Concurrent Threading
        </div>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {activeToolExecutions.map((exec) => (
          <motion.div
            key={exec.id}
            layout
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            transition={{ duration: 0.3 }}
            className={`relative flex flex-col gap-2 p-4 rounded-xl border backdrop-blur-md transition-all duration-300 shadow-xl ${
              exec.status === 'running'
                ? 'bg-blue-500/5 border-blue-500/20 shadow-blue-500/5'
                : exec.status === 'success'
                  ? 'bg-green-500/5 border-green-500/20 shadow-green-500/5'
                  : 'bg-red-500/5 border-red-500/20 shadow-red-500/5'
            }`}
          >
            {/* Glowing Background Accent */}
            <div className={`absolute top-0 right-0 w-24 h-24 rounded-full filter blur-[40px] opacity-10 pointer-events-none -mr-8 -mt-8 ${
              exec.status === 'running' ? 'bg-blue-500' : exec.status === 'success' ? 'bg-green-500' : 'bg-red-500'
            }`} />

            <div className="flex items-start justify-between">
              <div className="flex flex-col min-w-0">
                <span className="text-xs font-bold text-white font-mono flex items-center gap-1.5 truncate">
                  ⚙️ {exec.name}
                </span>
                {exec.args && (
                  <span className="text-[9px] text-muted-foreground font-mono truncate max-w-[180px] mt-0.5" title={exec.args}>
                    {exec.args}
                  </span>
                )}
              </div>
              <span className={`text-[9px] px-2 py-0.5 rounded-full font-bold uppercase shrink-0 ${
                exec.status === 'running' ? 'bg-blue-500/25 text-blue-400 animate-pulse' :
                exec.status === 'success' ? 'bg-green-500/25 text-green-400' :
                'bg-red-500/25 text-red-400'
              }`}>
                {exec.status}
              </span>
            </div>

            {/* Progress bar or output details */}
            {exec.status === 'running' ? (
              <div className="mt-2 flex flex-col gap-1">
                <div className="h-1 w-full bg-white/5 rounded-full overflow-hidden">
                  <motion.div
                    className="h-full bg-gradient-to-r from-blue-500 to-indigo-500 rounded-full"
                    initial={{ width: '15%' }}
                    animate={{ width: '90%' }}
                    transition={{ duration: 12, ease: 'easeOut' }}
                  />
                </div>
                <span className="text-[8px] text-blue-400/80 font-mono animate-pulse">
                  Streaming execution thread...
                </span>
              </div>
            ) : (
              <div className="mt-2 flex flex-col gap-1.5">
                <details className="text-[9px] text-muted-foreground font-mono bg-black/40 rounded border border-white/5 overflow-hidden">
                  <summary className="cursor-pointer py-1 px-2 select-none hover:bg-white/5 font-semibold text-[8px] text-white/60">
                    Show Log Output
                  </summary>
                  <div className="p-2 border-t border-white/5 max-h-32 overflow-y-auto whitespace-pre-wrap text-[9px] text-white/80 leading-normal select-text">
                    {exec.output || 'No output log.'}
                  </div>
                </details>
              </div>
            )}
          </motion.div>
        ))}
      </div>
    </div>
  );
}

export function AgentTimeline() {
  const { messages, isStreaming, addMessage, streamAccumulator, reasoningAccumulator, currentToolResults } = useStore();
  const [input, setInput] = useState('');
  const [rollbackConfirmId, setRollbackConfirmId] = useState<string | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = scrollContainerRef.current.scrollHeight;
    }
  }, [messages, isStreaming, streamAccumulator, reasoningAccumulator, currentToolResults]);

  const handleSend = () => {
    if (!input.trim() || isStreaming) return;
    addMessage({ id: Date.now().toString(), role: 'user', content: input });
    // @ts-ignore
    if (window.sendNexus) {
      let editorContext = undefined;
      const { activeFile } = useStore.getState();
      if (activeFile) {
        editorContext = `${activeFile.name}\n\nFile Contents:\n\`\`\`${activeFile.ext}\n${activeFile.content}\n\`\`\``;
      }
      // @ts-ignore
      window.sendNexus('Chat', { message: input, editor_context: editorContext });
      useStore.getState().clearActiveToolExecutions();
      useStore.getState().setStreaming(true);
    }
    setInput('');
  };

  const handleStop = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('StopStream', {});
    }
    useStore.getState().commitStream();
  };

  const handleRollback = (targetMsg: any) => {
    const userMessages = messages.filter(m => m.role === 'user');
    const userMsgIndex = userMessages.findIndex(m => m.id === targetMsg.id);
    
    if (userMsgIndex !== -1) {
      const targetIdx = messages.findIndex(m => m.id === targetMsg.id);
      if (targetIdx !== -1) {
        const truncated = messages.slice(0, targetIdx + 1);
        useStore.getState().setMessages(truncated);
      }
      
      // @ts-ignore
      if (window.sendNexus) {
        // @ts-ignore
        window.sendNexus('RollbackHistory', { user_message_index: userMsgIndex });
      }
    }
    setRollbackConfirmId(null);
  };

  return (
    <div className="flex flex-col h-full w-full bg-background relative">
      <div 
        ref={scrollContainerRef}
        className="flex-1 overflow-y-auto p-6 flex flex-col relative"
      >
        {/* The continuous vertical timeline line */}
        <div className="absolute left-10 top-6 bottom-6 w-0.5 bg-white/10 z-0"></div>

        <div className="flex flex-col gap-6 z-10 relative">
          <AnimatePresence>
            {messages.map((msg) => (
              <motion.div
                key={msg.id}
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                className="flex gap-4 relative group"
              >
                {/* Timeline Node Icon */}
                <div className={`flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-background border-2 shadow-sm z-10 
                  ${msg.role === 'user' ? 'border-blue-500 text-blue-500' : msg.role === 'system' ? 'border-accent text-accent' : 'border-green-500 text-green-500'}
                `}>
                  {msg.role === 'user' ? <CircleDashed size={16} /> : msg.role === 'system' ? <Cpu size={16} /> : <CheckCircle2 size={16} />}
                </div>

                <div className={`flex-1 flex flex-col gap-2 relative ${msg.role === 'user' ? 'pt-1' : ''}`}>
                  
                  {/* Rewind Action Hover Trigger */}
                  {msg.role === 'user' && msg.id !== 'init' && (
                    <div className="absolute top-1 right-2 flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity z-20">
                      <button 
                        onClick={() => setRollbackConfirmId(msg.id)}
                        className="flex items-center gap-1.5 px-2 py-1 bg-accent/20 hover:bg-accent/40 border border-accent/30 rounded text-[10px] text-accent font-bold cursor-pointer transition-colors shadow-sm"
                        title="Rewind conversation to here"
                      >
                        <RotateCcw size={10} /> REWIND
                      </button>
                    </div>
                  )}

                  {/* Glassmorphic Rollback Confirmation Overlay */}
                  {rollbackConfirmId === msg.id && (
                    <div className="absolute inset-0 bg-background/90 backdrop-blur-md rounded-xl flex items-center justify-between px-6 z-30 border border-accent/30 animate-in fade-in duration-200">
                      <div className="flex items-center gap-2 text-xs font-mono text-white/90">
                        <RotateCcw size={14} className="text-accent animate-pulse" />
                        <span>Rewind session to this prompt? (LLM context will be reset).</span>
                      </div>
                      <div className="flex gap-2">
                        <button 
                          onClick={() => handleRollback(msg)}
                          className="bg-accent hover:bg-accent/90 text-background px-3 py-1 rounded text-xs font-bold transition-all cursor-pointer shadow-md"
                        >
                          CONFIRM
                        </button>
                        <button 
                          onClick={() => setRollbackConfirmId(null)}
                          className="bg-white/10 hover:bg-white/20 text-white px-3 py-1 rounded text-xs font-bold transition-all cursor-pointer border border-white/10"
                        >
                          CANCEL
                        </button>
                      </div>
                    </div>
                  )}

                  {msg.role === 'user' && (
                    <div className="text-sm font-semibold text-white/90">User</div>
                  )}
                  {msg.role === 'ai' && (
                    <div className="text-sm font-semibold text-green-400">Tempest Agent</div>
                  )}
                  {msg.role === 'system' && (
                    <div className="text-sm font-semibold text-accent">System</div>
                  )}

                  {msg.reasoning && (
                    <CollapsibleThought reasoning={msg.reasoning} />
                  )}

                  {msg.tools && msg.tools.length > 0 && (
                    <div className="flex flex-col gap-2 border-l-2 border-purple-500/50 pl-3 py-1">
                      {msg.tools.map((t, idx) => (
                        <CollapsibleTool key={idx} tool={t} />
                      ))}
                    </div>
                  )}

                  {msg.content && (
                    <div className={`text-sm leading-relaxed whitespace-pre-wrap p-4 rounded-xl border ${
                      msg.role === 'user' ? 'bg-blue-500/10 border-blue-500/20 text-white/90' :
                      msg.role === 'system' ? 'bg-accent/10 border-accent/20 font-mono text-accent' :
                      'bg-green-500/10 border-green-500/20 text-white/90'
                    }`}>
                      {msg.content}
                    </div>
                  )}
                </div>
              </motion.div>
            ))}
          </AnimatePresence>

          {/* Active Streaming Node */}
          {isStreaming && (
            <motion.div
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex gap-4 relative"
            >
              <div className="flex-shrink-0 w-8 h-8 rounded-full flex items-center justify-center bg-background border-2 shadow-sm z-10 border-accent text-accent animate-pulse">
                <Cpu size={16} />
              </div>
              <div className="flex-1 flex flex-col gap-2">
                <div className="text-sm font-semibold text-accent animate-pulse">Tempest Agent (Active)</div>
                
                {reasoningAccumulator && (
                  <CollapsibleThought reasoning={reasoningAccumulator} title="Thinking..." defaultOpen={true} />
                )}

                <ParallelToolsDashboard />

                {currentToolResults.length > 0 && (
                  <div className="flex flex-col gap-2 border-l-2 border-purple-500/50 pl-3 py-1">
                    {currentToolResults.map((t, idx) => (
                      <CollapsibleTool key={idx} tool={t} defaultOpen={true} />
                    ))}
                  </div>
                )}

                {streamAccumulator && (
                  <div className="text-sm leading-relaxed whitespace-pre-wrap p-4 rounded-xl border bg-accent/10 border-accent/20 text-white/90 animate-pulse">
                    {streamAccumulator}
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </div>
      </div>

      <div className="p-4 bg-black/20 border-t border-border/50 relative z-20">
        <div className="flex gap-3">
          <input 
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            className="flex-1 bg-white/5 border border-border/50 rounded-lg px-4 py-3 text-sm focus:outline-none focus:border-accent transition-colors shadow-inner text-white placeholder-muted-foreground"
            placeholder="Enter objective..."
            disabled={isStreaming}
          />
          <button 
            onClick={isStreaming ? handleStop : handleSend}
            disabled={!isStreaming && !input}
            className={`font-bold px-6 py-3 rounded-lg flex items-center justify-center transition-all shadow-lg hover:-translate-y-0.5 active:translate-y-0 ${
              isStreaming 
                ? 'bg-destructive hover:bg-destructive/90 text-white'
                : 'bg-accent hover:bg-accent/90 text-background'
            }`}
          >
            {isStreaming ? <Square size={18} className="fill-current" /> : <Send size={18} />}
          </button>
        </div>
      </div>
    </div>
  );
}
