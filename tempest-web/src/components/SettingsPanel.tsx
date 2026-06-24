import {
  Eye,
  Settings,
  Sliders,
  ShieldAlert,
  Sparkles,
  User,
  Zap,
  Terminal,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { useStore } from '../store';

export function SettingsPanel() {
  const {
    backgroundIntensity,
    setBackgroundIntensity,
    sliderAggressiveCareful,
    sliderCreativePrecise,
    sliderFastThorough,
    activeRole,
    contextLimit,
    muteSounds,
    setSliderAggressiveCareful,
    setSliderCreativePrecise,
    setSliderFastThorough,
    setActiveRole,
    setContextLimit,
    setMuteSounds,
  } = useStore();

  const intensities: { id: 'subtle' | 'medium' | 'full'; label: string }[] = [
    { id: 'subtle', label: 'Subtle' },
    { id: 'medium', label: 'Medium' },
    { id: 'full', label: 'Full' },
  ];

  const roles = [
    {
      id: 'pair-programmer',
      label: 'Pair Programmer',
      desc: 'Collaborative development assistant. Focused on step-by-step logic, clear designs, and code-review cooperation.',
      icon: <Terminal size={14} className="text-blue-400" />,
    },
    {
      id: 'senior-editor',
      label: 'Senior Editor',
      desc: 'Focused on codebase styling, structure, clean architecture, readability, and ensuring code quality rules are followed.',
      icon: <Sliders size={14} className="text-yellow-400" />,
    },
    {
      id: 'security-auditor',
      label: 'Security Auditor',
      desc: 'Focused on vetting inputs, checks sanitization, and hunting for injection, race conditions, or execution vulnerability vectors.',
      icon: <ShieldAlert size={14} className="text-red-400" />,
    },
    {
      id: 'code-poet',
      label: 'Code Poet',
      desc: 'Focused on highly elegant, expressive, and self-documenting code with beautifully styled comments.',
      icon: <Sparkles size={14} className="text-purple-400" />,
    },
    {
      id: 'refactor-ninja',
      label: 'Refactor Ninja',
      desc: 'Focused on cleaning up duplicates, simplifying cognitive load, optimizing speed, and refactoring with surgical changes.',
      icon: <Zap size={14} className="text-green-400" />,
    },
  ];

  // Weight combination mapping to final temperature
  const computedTemp = Math.max(
    0.01,
    sliderCreativePrecise * 1.0 + sliderAggressiveCareful * 0.6 + sliderFastThorough * 0.4
  );

  return (
    <div className="flex flex-col gap-6 text-sm h-full overflow-y-auto pr-1">
      {/* Background Intensity */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <Eye size={12} /> Visual Effects
        </h4>
        <p className="text-xs text-muted-foreground mb-1">
          Set background Vortex canvas visibility.
        </p>
        <div className="grid grid-cols-3 gap-2">
          {intensities.map((item) => (
            <button
              key={item.id}
              onClick={() => setBackgroundIntensity(item.id)}
              className={`py-2 px-3 text-xs font-semibold rounded-md border transition-all cursor-pointer ${
                backgroundIntensity === item.id
                  ? 'bg-accent border-accent text-background shadow-md font-bold'
                  : 'bg-white/5 border-border hover:bg-white/10 text-muted-foreground'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>

      {/* UI Sounds */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <Volume2 size={12} /> Audio Effects
        </h4>
        <div className="flex items-center justify-between bg-white/5 p-3 rounded-lg border border-white/5">
          <div className="flex flex-col">
            <span className="text-xs font-bold text-white">System Sounds</span>
            <span className="text-[10px] text-muted-foreground">
              Synthesized audio cues for UI interactions
            </span>
          </div>
          <button
            onClick={() => setMuteSounds(!muteSounds)}
            className={`px-3 py-1.5 text-xs font-semibold rounded-md border transition-all cursor-pointer flex items-center gap-2 ${
              !muteSounds
                ? 'bg-accent/10 border-accent text-accent shadow-[0_0_10px_rgba(0,242,255,0.15)]'
                : 'bg-white/5 border-border hover:bg-white/10 text-muted-foreground'
            }`}
          >
            {!muteSounds ? <Volume2 size={14} /> : <VolumeX size={14} />}
            {!muteSounds ? 'ENABLED' : 'MUTED'}
          </button>
        </div>
      </div>

      {/* Preset Agent Personas */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <User size={12} /> Preset Agent Personas
        </h4>
        <p className="text-xs text-muted-foreground mb-1">
          Select the operational role guidelines for the agent.
        </p>
        <div className="flex flex-col gap-2">
          {roles.map((role) => (
            <button
              key={role.id}
              onClick={() => setActiveRole(role.id as any)}
              className={`p-3 rounded-lg border text-left transition-all duration-300 flex flex-col gap-1 cursor-pointer relative overflow-hidden group ${
                activeRole === role.id
                  ? 'bg-accent/10 border-accent text-white shadow-[0_0_15px_rgba(0,242,255,0.08)]'
                  : 'bg-white/5 border-white/5 hover:bg-white/10 hover:border-white/10 text-muted-foreground'
              }`}
            >
              {activeRole === role.id && (
                <span className="absolute top-0 left-0 w-1 h-full bg-accent" />
              )}
              <div className="flex items-center gap-2 font-bold text-xs text-white">
                {role.icon}
                <span className="group-hover:text-accent transition-colors">{role.label}</span>
              </div>
              <span className="text-[11px] opacity-70 leading-normal font-sans">{role.desc}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Inference & Temperature Settings */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <Sliders size={12} /> Inference Settings
        </h4>
        <div className="bg-white/5 p-4 rounded-lg border border-white/5 flex flex-col gap-4">
          {/* Dynamic calculated Temperature */}
          <div className="flex justify-between items-center bg-accent/10 border border-accent/20 rounded-md p-2.5 font-mono">
            <span className="text-xs text-white font-semibold">Calculated Temperature:</span>
            <span className="text-accent text-sm font-bold shadow-neon">
              {computedTemp.toFixed(2)}
            </span>
          </div>

          {/* Slider 1: Creative vs Precise */}
          <div>
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Creative vs Precise</span>
              <span className="text-white/80 text-[10px]">
                {(sliderCreativePrecise * 100).toFixed(0)}%
              </span>
            </div>
            <div className="flex items-center gap-2 text-[10px] text-muted-foreground mb-1">
              <span>Precise</span>
              <input
                type="range"
                min="0.0"
                max="1.0"
                step="0.05"
                value={sliderCreativePrecise}
                onChange={(e) => setSliderCreativePrecise(parseFloat(e.target.value))}
                className="flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
              />
              <span>Creative</span>
            </div>
          </div>

          {/* Slider 2: Aggressive vs Careful */}
          <div>
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Aggressive vs Careful</span>
              <span className="text-white/80 text-[10px]">
                {(sliderAggressiveCareful * 100).toFixed(0)}%
              </span>
            </div>
            <div className="flex items-center gap-2 text-[10px] text-muted-foreground mb-1">
              <span>Careful</span>
              <input
                type="range"
                min="0.0"
                max="1.0"
                step="0.05"
                value={sliderAggressiveCareful}
                onChange={(e) => setSliderAggressiveCareful(parseFloat(e.target.value))}
                className="flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
              />
              <span>Aggressive</span>
            </div>
          </div>

          {/* Slider 3: Fast vs Thorough */}
          <div>
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Fast vs Thorough</span>
              <span className="text-white/80 text-[10px]">
                {(sliderFastThorough * 100).toFixed(0)}%
              </span>
            </div>
            <div className="flex items-center gap-2 text-[10px] text-muted-foreground mb-1">
              <span>Thorough</span>
              <input
                type="range"
                min="0.0"
                max="1.0"
                step="0.05"
                value={sliderFastThorough}
                onChange={(e) => setSliderFastThorough(parseFloat(e.target.value))}
                className="flex-1 accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
              />
              <span>Fast</span>
            </div>
          </div>

          {/* Context Limit */}
          <div className="border-t border-white/5 pt-3 mt-1">
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Context Limit</span>
              <span className="text-accent">{contextLimit} tokens</span>
            </div>
            <input
              type="range"
              min="2048"
              max="32768"
              step="1024"
              value={contextLimit}
              onChange={(e) => setContextLimit(parseInt(e.target.value))}
              className="w-full accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
            />
          </div>
        </div>
      </div>

      {/* Extra Info */}
      <div className="p-3 bg-accent/5 rounded-lg border border-accent/15 flex flex-col gap-2 text-xs text-muted-foreground font-mono mb-4">
        <div className="flex items-center gap-2 text-accent font-semibold">
          <Settings size={12} /> System Diagnostics
        </div>
        <div>Config: local/config.toml</div>
        <div>UI Frame: React 19.2 + Vite</div>
        <div>Persona: {activeRole}</div>
      </div>
    </div>
  );
}
