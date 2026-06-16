import { useState } from 'react';
import { useStore } from '../store';
import { MessageCircleQuestion, Send } from 'lucide-react';
import { motion } from 'framer-motion';

export function AskUserModal() {
  const { askUserRequest, setAskUserRequest } = useStore();
  const [answer, setAnswer] = useState('');

  if (!askUserRequest) return null;

  const handleSubmit = (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('AskUserResponse', { answer });
    }
    setAskUserRequest(null);
    setAnswer('');
  };

  return (
    <div className="fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/60 backdrop-blur-md">
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        className="w-[600px] max-w-full glass-panel border border-border/80 rounded-2xl shadow-[0_0_50px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col max-h-[85vh]"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40">
          <div className="flex items-center gap-3 text-accent">
            <MessageCircleQuestion className="drop-shadow-[0_0_8px_rgba(0,242,255,0.3)]" />
            <div>
              <h3 className="font-semibold text-sm tracking-wider">AGENT QUESTION</h3>
              <p className="text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5">Clarification Required</p>
            </div>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 p-6 flex flex-col gap-4">
          {/* Question */}
          <div className="bg-white/5 border border-white/5 p-4 rounded-xl">
            <p className="text-sm leading-relaxed text-white whitespace-pre-wrap">{askUserRequest.question}</p>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="mt-2 relative">
            <textarea
              autoFocus
              value={answer}
              onChange={(e) => setAnswer(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSubmit();
                }
              }}
              placeholder="Type your response here..."
              className="w-full h-[120px] bg-black/40 border border-border/50 rounded-xl p-4 text-sm resize-none focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/50 text-foreground placeholder:text-muted-foreground transition-all"
            />
            <div className="absolute bottom-3 right-3 text-[10px] text-muted-foreground/60 select-none pointer-events-none">
              Press Enter to send, Shift+Enter for newline
            </div>
          </form>
        </div>

        {/* Footer */}
        <div className="flex justify-end items-center px-6 py-4 border-t border-border/50 bg-black/40 flex-none">
          <button
            onClick={() => handleSubmit()}
            disabled={!answer.trim()}
            className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] disabled:opacity-50 disabled:cursor-not-allowed disabled:shadow-none disabled:active:translate-y-0 cursor-pointer"
          >
            <Send size={14} /> Send Response
          </button>
        </div>
      </motion.div>
    </div>
  );
}
