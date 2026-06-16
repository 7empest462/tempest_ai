import { useState } from 'react';
import { useStore } from '../store';
import { Sliders, Eye, Settings } from 'lucide-react';

export function SettingsPanel() {
  const { backgroundIntensity, setBackgroundIntensity } = useStore();
  const [temperature, setTemperature] = useState(0.7);
  const [contextWindow, setContextWindow] = useState(32768);

  const intensities: { id: 'subtle' | 'medium' | 'full'; label: string }[] = [
    { id: 'subtle', label: 'Subtle' },
    { id: 'medium', label: 'Medium' },
    { id: 'full', label: 'Full' }
  ];

  return (
    <div className="flex flex-col gap-6 text-sm">
      {/* Background Intensity */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <Eye size={12} /> Visual Effects
        </h4>
        <p className="text-xs text-muted-foreground mb-1">Set background Vortex canvas visibility.</p>
        <div className="grid grid-cols-3 gap-2">
          {intensities.map((item) => (
            <button
              key={item.id}
              onClick={() => setBackgroundIntensity(item.id)}
              className={`py-2 px-3 text-xs font-semibold rounded-md border transition-all cursor-pointer ${
                backgroundIntensity === item.id
                  ? 'bg-accent border-accent text-background shadow-md'
                  : 'bg-white/5 border-border hover:bg-white/10 text-muted-foreground'
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>

      {/* Temperature */}
      <div className="flex flex-col gap-2">
        <h4 className="text-[10px] uppercase font-bold tracking-wider text-muted-foreground flex items-center gap-1.5">
          <Sliders size={12} /> Inference Settings
        </h4>
        <div className="bg-white/5 p-3 rounded-lg border border-white/5 flex flex-col gap-3">
          <div>
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Temperature</span>
              <span className="text-accent">{temperature.toFixed(2)}</span>
            </div>
            <input
              type="range"
              min="0.0"
              max="2.0"
              step="0.05"
              value={temperature}
              onChange={(e) => setTemperature(parseFloat(e.target.value))}
              className="w-full accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
            />
          </div>

          <div>
            <div className="flex justify-between items-center text-xs font-mono mb-1 text-muted-foreground">
              <span>Context Limit</span>
              <span className="text-accent">{contextWindow} tokens</span>
            </div>
            <input
              type="range"
              min="2048"
              max="32768"
              step="1024"
              value={contextWindow}
              onChange={(e) => setContextWindow(parseInt(e.target.value))}
              className="w-full accent-accent h-1 bg-white/10 rounded-lg appearance-none cursor-pointer"
            />
          </div>
        </div>
      </div>

      {/* Extra Info */}
      <div className="p-3 bg-accent/5 rounded-lg border border-accent/15 flex flex-col gap-2 text-xs text-muted-foreground font-mono">
        <div className="flex items-center gap-2 text-accent font-semibold">
          <Settings size={12} /> System Diagnostics
        </div>
        <div>Config: local/config.toml</div>
        <div>UI Frame: React 19.2 + Vite</div>
        <div>VRAM Policy: Multi-Model Shared</div>
      </div>
    </div>
  );
}
