import { useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useStore } from '../store';
import { Send, Square } from 'lucide-react';

export function ChatInterface() {
  const {
    messages,
    isStreaming,
    addMessage,
    streamAccumulator,
    reasoningAccumulator,
    currentToolResults,
    activeFile,
    agentPhase,
  } = useStore();
  const [input, setInput] = useState('');
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollTop = scrollContainerRef.current.scrollHeight;
    }
  }, [
    messages,
    isStreaming,
    streamAccumulator,
    reasoningAccumulator,
    currentToolResults,
    agentPhase,
  ]);

  const handleSend = () => {
    if (!input.trim() || isStreaming) return;

    addMessage({ id: Date.now().toString(), role: 'user', content: input });

    // @ts-ignore
    if (window.sendNexus) {
      let editorContext = undefined;
      if (activeFile) {
        editorContext = `${activeFile.name}\n\nFile Contents:\n\`\`\`${activeFile.ext}\n${activeFile.content}\n\`\`\``;
      }

      const state = useStore.getState();
      const calculatedTemp = Math.max(
        0.01,
        state.sliderCreativePrecise * 1.0 +
          state.sliderAggressiveCareful * 0.6 +
          state.sliderFastThorough * 0.4
      );

      // @ts-ignore
      window.sendNexus('Chat', {
        message: input,
        editor_context: editorContext,
        temperature: calculatedTemp,
        context_limit: state.contextLimit,
        role: state.activeRole,
      });
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

  return (
    <div className="flex flex-col h-full w-full">
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto p-4 flex flex-col gap-4">
        <AnimatePresence>
          {messages.map((msg) => (
            <motion.div
              key={msg.id}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className={`max-w-[85%] p-4 rounded-xl text-sm leading-relaxed ${
                msg.role === 'system'
                  ? 'bg-accent/10 border border-accent/20 font-mono self-start'
                  : msg.role === 'ai'
                    ? 'bg-white/5 border border-white/10 self-start'
                    : 'bg-[rgba(112,0,255,0.2)] border border-[rgba(112,0,255,0.4)] self-end'
              }`}
            >
              {msg.role === 'system' && <span className="mr-2">⚡</span>}
              {msg.reasoning && (
                <details className="text-xs text-muted-foreground border-l-2 border-accent/50 pl-2 mb-2">
                  <summary className="cursor-pointer font-semibold select-none hover:text-white transition-colors">
                    Thought Process
                  </summary>
                  <div className="mt-1 font-mono whitespace-pre-wrap opacity-70">
                    {msg.reasoning}
                  </div>
                </details>
              )}
              {msg.tools && msg.tools.length > 0 && (
                <div className="flex flex-col gap-2 mb-2">
                  {msg.tools.map((t, idx) => (
                    <details key={idx} className="bg-black/20 border border-white/5 rounded block">
                      <summary className="text-xs cursor-pointer select-none py-1 px-2 text-purple-400 font-semibold hover:bg-white/5 transition-colors">
                        🔧 Tool: {t.name} {t.success ? '✅' : '❌'}
                      </summary>
                      <div className="p-2 border-t border-white/5 text-[10px] font-mono">
                        {t.args && (
                          <div className="mb-1">
                            <strong className="text-muted-foreground">Input:</strong>
                            <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto">
                              {t.args}
                            </pre>
                          </div>
                        )}
                        {t.output && (
                          <div>
                            <strong className="text-muted-foreground">Output:</strong>
                            <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto max-h-40">
                              {t.output}
                            </pre>
                          </div>
                        )}
                      </div>
                    </details>
                  ))}
                </div>
              )}
              {msg.content && <div className="whitespace-pre-wrap">{msg.content}</div>}
            </motion.div>
          ))}
        </AnimatePresence>

        {/* Real-time Streaming & Reasoning Indicators */}
        {isStreaming &&
          (reasoningAccumulator || streamAccumulator || currentToolResults.length > 0) && (
            <div className="max-w-[85%] p-4 rounded-xl text-sm leading-relaxed bg-white/5 border border-white/10 self-start flex flex-col gap-2">
              {reasoningAccumulator && (
                <details
                  open
                  className="text-xs text-muted-foreground border-l-2 border-accent/50 pl-2 mb-1"
                >
                  <summary className="cursor-pointer font-semibold select-none hover:text-white transition-colors">
                    Thinking Process
                  </summary>
                  <div className="mt-1 font-mono whitespace-pre-wrap opacity-70">
                    {reasoningAccumulator}
                  </div>
                </details>
              )}
              {currentToolResults.length > 0 && (
                <div className="flex flex-col gap-2 mb-1">
                  {currentToolResults.map((t, idx) => (
                    <details
                      key={idx}
                      open
                      className="bg-black/20 border border-white/5 rounded block"
                    >
                      <summary className="text-xs cursor-pointer select-none py-1 px-2 text-purple-400 font-semibold hover:bg-white/5 transition-colors">
                        🔧 Tool: {t.name} {t.success ? '✅' : '❌'}
                      </summary>
                      <div className="p-2 border-t border-white/5 text-[10px] font-mono">
                        {t.args && (
                          <div className="mb-1">
                            <strong className="text-muted-foreground">Input:</strong>
                            <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto">
                              {t.args}
                            </pre>
                          </div>
                        )}
                        {t.output && (
                          <div>
                            <strong className="text-muted-foreground">Output:</strong>
                            <pre className="whitespace-pre-wrap text-white/70 overflow-x-auto max-h-40">
                              {t.output}
                            </pre>
                          </div>
                        )}
                      </div>
                    </details>
                  ))}
                </div>
              )}
              {streamAccumulator && <div className="whitespace-pre-wrap">{streamAccumulator}</div>}
            </div>
          )}
        {agentPhase === 'Compacting' && (
          <div className="max-w-[85%] p-4 rounded-xl text-sm leading-relaxed bg-white/5 border border-white/10 self-start flex items-center gap-3">
            <span className="animate-spin text-base">🌪️</span>
            <span className="text-purple-400 font-mono font-bold animate-pulse">
              agent is compacting history. Please wait one moment.
            </span>
          </div>
        )}
      </div>

      <div className="p-4 bg-black/20 border-t border-border/50">
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
