import { useState } from 'react';
import { motion } from 'framer-motion';
import { useStore } from '../store';
import type { AgentPhase, WebMemoryItem } from '../store';
import { Brain, ClipboardList, Wrench, Search, RefreshCw, ChevronDown, Calendar, Tag, Copy, Check } from 'lucide-react';

const phases = [
  { id: 'Thinking' as AgentPhase, icon: Brain, label: 'Thinking' },
  { id: 'Planning' as AgentPhase, icon: ClipboardList, label: 'Planning' },
  { id: 'Executing' as AgentPhase, icon: Wrench, label: 'Executing' },
];

function getFriendlyTitle(topic: string): string {
  // 1. Technical slugs mapping
  const technicalMapping: Record<string, string> = {
    tool_routing_stocks: 'Rule: Stock Pricing',
    tool_routing_http: 'Rule: HTTP Fetching',
    tool_routing_network: 'Rule: Network Operations',
    tool_routing_memory_search: 'Rule: Memory Searches',
    tool_routing_hallucination: 'Rule: Tool Verification',
    task_completion: 'Rule: Task Completion',
    tempest_identity: 'Agent Persona',
    code_quality_guideline: 'Rule: Code Quality',
    context_management: 'Rule: Context Management',
    file_modification_safety: 'Rule: Change Safety',
  };

  if (technicalMapping[topic]) {
    return technicalMapping[topic];
  }

  // 2. Clean up "File `src/skills.rs` indexed successfully..." -> "Index: src/skills.rs"
  if (topic.toLowerCase().startsWith('file ') && topic.toLowerCase().includes('indexed successfully')) {
    const match = topic.match(/`([^`]+)`/);
    if (match) {
      return `Index: ${match[1]}`;
    }
  }

  // 3. Clean up standard skill titles
  if (topic.startsWith('Skill: ')) {
    return topic;
  }

  // 4. Clean up standard project file context
  if (topic.startsWith('Rust Project: ')) {
    return topic;
  }

  // 5. Shorten generic topic
  if (topic.length > 40) {
    const splitIndex = topic.search(/[:,.]/);
    if (splitIndex > 5 && splitIndex < 35) {
      return topic.substring(0, splitIndex).trim();
    }
    return topic.substring(0, 37).trim() + '...';
  }

  return topic;
}

function MemoryCard({ mem }: { mem: WebMemoryItem }) {
  const [isOpen, setIsOpen] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(mem.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const friendlyTitle = getFriendlyTitle(mem.topic);

  const getExcerpt = (text: string) => {
    let clean = text;
    // Strip technical prefixes for preview readability
    clean = clean.replace(/^CORE INSTRUCTION \([^)]+\):\s*/i, '');
    clean = clean.replace(/^CORE INSTRUCTION:\s*/i, '');

    if (clean.length > 95) {
      return clean.substring(0, 92) + '...';
    }
    return clean;
  };

  const contentPreview = getExcerpt(mem.content);

  return (
    <div
      className={`shrink-0 bg-white/[0.02] border border-white/5 rounded-xl overflow-hidden hover:bg-white/[0.04] transition-all duration-200 border-l-2 ${isOpen ? 'border-l-accent bg-white/[0.04] shadow-[0_0_15px_rgba(0,242,255,0.05)]' : 'border-l-purple-500/30'
        }`}
    >
      {/* Card Header (Clickable Tab) */}
      <div
        onClick={() => setIsOpen(!isOpen)}
        className="p-3.5 flex items-start gap-3 cursor-pointer justify-between select-none min-h-[52px]"
      >
        <div className="flex-1 min-w-0 flex flex-col gap-1.5">
          <div className="flex items-center justify-between gap-2">
            <span className="font-bold text-[12px] text-white/95 tracking-wide font-mono truncate">
              {friendlyTitle}
            </span>
            <span className="text-[8px] text-muted-foreground font-mono shrink-0">
              {new Date(mem.updated_at).toLocaleDateString()}
            </span>
          </div>

          {/* Content Excerpt (visible when collapsed) */}
          {!isOpen && (
            <p className="text-[10px] text-muted-foreground/85 leading-normal break-words font-sans">
              {contentPreview}
            </p>
          )}

          {mem.tags && (
            <div className="flex flex-wrap gap-1 mt-0.5">
              {mem.tags.split(',').map(tag => (
                <span key={tag} className="text-[8px] font-mono bg-purple-500/10 text-purple-400 border border-purple-500/20 px-1.5 py-0.5 rounded-full flex items-center gap-0.5">
                  <Tag size={8} /> {tag.trim()}
                </span>
              ))}
            </div>
          )}
        </div>
        <motion.div
          animate={{ rotate: isOpen ? 180 : 0 }}
          transition={{ duration: 0.15 }}
          className="text-muted-foreground mt-0.5 shrink-0"
        >
          <ChevronDown size={14} />
        </motion.div>
      </div>

      {/* Card Body (CSS Grid Transition for reliable height expansion) */}
      <div
        className={`grid transition-all duration-200 ease-in-out ${isOpen ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0 pointer-events-none'
          }`}
      >
        <div className="overflow-hidden">
          <div className="px-3.5 pb-3.5 border-t border-white/5 bg-black/25 flex flex-col gap-3 text-xs font-mono select-text">
            <div className="text-white/80 whitespace-pre-wrap leading-relaxed break-words py-2.5 px-3 bg-black/35 rounded-lg border border-white/5 text-[11px] mt-2">
              {mem.content}
            </div>

            <div className="flex items-center justify-between text-[8px] text-muted-foreground/60 select-none pt-1">
              <span className="flex items-center gap-1 font-sans">
                <Calendar size={8} /> Updated: {new Date(mem.updated_at).toLocaleString()}
              </span>
              <button
                onClick={handleCopy}
                className="flex items-center gap-1.5 hover:text-white transition-all py-1 px-2 rounded bg-white/5 border border-white/5 hover:border-white/10 cursor-pointer font-bold font-sans"
                title="Copy memory content"
              >
                {copied ? (
                  <>
                    <Check size={8} className="text-green-400" />
                    <span className="text-green-400 font-sans">COPIED</span>
                  </>
                ) : (
                  <>
                    <Copy size={8} />
                    <span className="font-sans">COPY</span>
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}


export function MissionControl() {
  const {
    agentPhase,
    currentTask,
    activeTools,
    memories,
  } = useStore();
  const [searchQuery, setSearchQuery] = useState('');

  const isActive = (phaseId: AgentPhase) => {
    if (phaseId === 'Planning' && agentPhase === 'PendingTools') return true;
    if (phaseId === 'Executing' && agentPhase === 'ExecutingTools') return true;
    return agentPhase === phaseId;
  };

  const handleRefresh = () => {
    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('GetMemories', {});
    }
  };

  const filteredMemories = memories.filter(m =>
    m.topic.toLowerCase().includes(searchQuery.toLowerCase()) ||
    m.content.toLowerCase().includes(searchQuery.toLowerCase()) ||
    (m.tags && m.tags.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  return (
    <div className="flex flex-col gap-5 select-text h-full max-h-full overflow-hidden">
      {/* Stepper */}
      <div className="flex justify-between items-center px-2 relative flex-none">
        <div className="absolute top-5 left-8 right-8 h-0.5 bg-border -z-10" />
        {phases.map((phase) => {
          const active = isActive(phase.id);
          return (
            <div key={phase.id} className="flex flex-col items-center gap-2 bg-transparent">
              <motion.div
                animate={{
                  scale: active ? 1.15 : 1,
                  boxShadow: active ? '0 0 15px rgba(0,242,255,0.4)' : 'none'
                }}
                className={`w-10 h-10 rounded-full flex items-center justify-center transition-colors ${active ? 'bg-accent text-background' : 'glass-panel border border-border text-muted-foreground'
                  }`}
              >
                <phase.icon size={18} />
              </motion.div>
              <span className={`text-[10px] uppercase font-bold tracking-wider ${active ? 'text-accent' : 'text-muted-foreground'}`}>
                {phase.label}
              </span>
            </div>
          );
        })}
      </div>


      {/* Task */}
      {currentTask !== '--' && (
        <div className="bg-white/5 border border-white/10 p-3 rounded-lg flex-none">
          <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-1">Current Task</h4>
          <p className="text-xs font-mono text-white leading-normal">{currentTask}</p>
        </div>
      )}

      {/* Active Tools */}
      <div className="flex-none">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground mb-2">Active Tools</h4>
        <div className="flex flex-col gap-2">
          {activeTools.length === 0 ? (
            <span className="text-xs italic text-muted-foreground border border-transparent p-1">No tools running</span>
          ) : (
            activeTools.map((tool, idx) => (
              <motion.div
                key={idx}
                initial={{ opacity: 0, x: -10 }}
                animate={{ opacity: 1, x: 0 }}
                className="bg-accent/10 border border-accent/30 px-3 py-2 rounded-md flex items-center gap-2 text-xs font-mono text-accent"
              >
                <span className="animate-spin text-[10px]">⚙️</span> {tool}
              </motion.div>
            ))
          )}
        </div>
      </div>

      {/* Divider */}
      <div className="border-t border-border/40 flex-none" />

      {/* Memory Inspector / The Brain */}
      <div className="flex-1 min-h-0 flex flex-col gap-3 overflow-hidden">
        <div className="flex items-center justify-between flex-none">
          <div className="flex items-center gap-2">
            <Brain size={14} className="text-accent" />
            <h4 className="text-[10px] uppercase font-bold tracking-wider text-white">THE BRAIN</h4>
          </div>
          <button
            onClick={handleRefresh}
            className="p-1 hover:bg-white/5 border border-transparent hover:border-white/10 rounded transition-all cursor-pointer text-muted-foreground hover:text-white"
            title="Refresh memory store"
          >
            <RefreshCw size={12} />
          </button>
        </div>

        {/* Search */}
        <div className="relative flex-none">
          <Search size={12} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search agent memories..."
            className="w-full bg-white/5 border border-border/50 rounded-lg pl-8 pr-4 py-2 text-xs focus:outline-none focus:border-accent transition-colors text-white placeholder-muted-foreground"
          />
        </div>

        {/* Memory list */}
        <div className="flex-1 overflow-y-auto pr-1 flex flex-col gap-2 min-h-0 scroll-smooth">
          {filteredMemories.length === 0 ? (
            <div className="text-center py-6 text-xs text-muted-foreground italic border border-white/5 rounded-xl bg-white/[0.01]">
              No memories found matching filter.
            </div>
          ) : (
            filteredMemories.map((mem) => (
              <MemoryCard key={mem.topic} mem={mem} />
            ))
          )}
        </div>
      </div>
    </div>
  );
}
