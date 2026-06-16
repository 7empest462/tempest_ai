import { useStore } from '../store';
import { ShieldAlert, Check, X } from 'lucide-react';
import { motion } from 'framer-motion';

export function SafeModeModal() {
  const { safeModeRequest, setSafeModeRequest } = useStore();

  if (!safeModeRequest) return null;

  const handleApprove = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('SafeModeApprove', {});
    }
    setSafeModeRequest(null);
  };

  const handleReject = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('SafeModeReject', {});
    }
    setSafeModeRequest(null);
  };

  return (
    <div className="fixed inset-0 z-[150] flex items-center justify-center p-6 bg-black/60 backdrop-blur-md">
      <motion.div
        initial={{ opacity: 0, scale: 0.95 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.95 }}
        className="w-[850px] max-w-full glass-panel border border-border/80 rounded-2xl shadow-[0_0_50px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col max-h-[85vh]"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border/50 bg-black/40">
          <div className="flex items-center gap-3 text-amber-400">
            <ShieldAlert className="drop-shadow-[0_0_8px_rgba(251,191,36,0.3)]" />
            <div>
              <h3 className="font-semibold text-sm tracking-wider">APPROVAL REQUIRED</h3>
              <p className="text-[10px] text-muted-foreground uppercase font-bold tracking-widest mt-0.5">Safe Mode Sentinel Gate</p>
            </div>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 p-6 overflow-y-auto flex flex-col gap-4 min-h-0">
          {/* Rationale */}
          <div className="bg-white/5 border border-white/5 p-4 rounded-xl">
            <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1">Proposed Rationale</h4>
            <p className="text-sm leading-relaxed text-white">{safeModeRequest.rationale}</p>
          </div>

          {/* Diff */}
          <div className="flex-1 flex flex-col gap-1 min-h-0">
            <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1">Code Diff</h4>
            <div className="flex-1 bg-black/40 border border-border/50 rounded-xl overflow-auto p-4 max-h-[45vh] font-mono text-xs select-text leading-relaxed">
              {safeModeRequest.diff ? (
                safeModeRequest.diff.split('\n').map((line, idx) => {
                  let lineClass = 'text-muted-foreground/80';
                  if (line.startsWith('+')) {
                    lineClass = 'text-green-400 bg-green-500/10 px-1.5 rounded-sm block w-full py-0.5';
                  } else if (line.startsWith('-')) {
                    lineClass = 'text-red-400 bg-red-500/10 px-1.5 rounded-sm block w-full py-0.5';
                  } else if (line.startsWith('@@')) {
                    lineClass = 'text-accent/80 font-bold block py-1 border-t border-white/5 mt-1';
                  }
                  return (
                    <span key={idx} className={lineClass}>
                      {line}
                    </span>
                  );
                })
              ) : (
                <span className="italic text-muted-foreground">No diff payload provided (Permission Escalation)</span>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end items-center gap-4 px-6 py-4 border-t border-border/50 bg-black/40 flex-none">
          <button
            onClick={handleReject}
            className="flex items-center gap-2 px-5 py-2.5 rounded-lg border border-red-500/30 bg-red-500/10 text-red-400 text-xs font-bold uppercase tracking-wider hover:bg-red-500/20 active:translate-y-px transition-all cursor-pointer"
          >
            <X size={14} /> Reject
          </button>
          <button
            onClick={handleApprove}
            className="flex items-center gap-2 px-5 py-2.5 rounded-lg bg-accent text-background text-xs font-bold uppercase tracking-wider hover:bg-accent/90 active:translate-y-px transition-all shadow-[0_0_15px_rgba(0,242,255,0.2)] cursor-pointer"
          >
            <Check size={14} /> Approve
          </button>
        </div>
      </motion.div>
    </div>
  );
}
