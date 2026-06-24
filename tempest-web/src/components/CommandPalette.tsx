import { useEffect, useState } from 'react';
import { Command } from 'cmdk';
import { useStore } from '../store';

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const { setBackgroundIntensity, backgroundIntensity } = useStore();

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((open) => !open);
      }
    };
    document.addEventListener('keydown', down);
    return () => document.removeEventListener('keydown', down);
  }, []);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[100] flex items-start justify-center pt-[15vh] bg-black/40 backdrop-blur-sm"
      onClick={() => setOpen(false)}
    >
      <div
        className="w-[600px] max-w-full glass-panel border border-border/70 rounded-xl shadow-[0_0_40px_rgba(0,0,0,0.6)] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <Command
          label="Global Command Menu"
          className="w-full text-foreground bg-transparent"
          shouldFilter={true}
        >
          <Command.Input
            autoFocus
            className="w-full bg-transparent px-4 py-4 border-b border-border/50 focus:outline-none placeholder:text-muted-foreground text-[15px]"
            placeholder="Type a command or search... (e.g. 'background')"
          />
          <Command.List className="p-2 max-h-[400px] overflow-y-auto">
            <Command.Empty className="p-6 text-center text-muted-foreground text-sm">
              No results found.
            </Command.Empty>

            <Command.Group
              heading="Settings: Appearance"
              className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1 px-3 pt-3"
            >
              <Command.Item
                onSelect={() => {
                  setBackgroundIntensity('subtle');
                  setOpen(false);
                }}
                className="flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors"
              >
                Set Background Intensity: Subtle{' '}
                {backgroundIntensity === 'subtle' && <span className="ml-2 text-accent">✓</span>}
              </Command.Item>
              <Command.Item
                onSelect={() => {
                  setBackgroundIntensity('medium');
                  setOpen(false);
                }}
                className="flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors"
              >
                Set Background Intensity: Medium{' '}
                {backgroundIntensity === 'medium' && <span className="ml-2 text-accent">✓</span>}
              </Command.Item>
              <Command.Item
                onSelect={() => {
                  setBackgroundIntensity('full');
                  setOpen(false);
                }}
                className="flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors"
              >
                Set Background Intensity: Full{' '}
                {backgroundIntensity === 'full' && <span className="ml-2 text-accent">✓</span>}
              </Command.Item>
            </Command.Group>

            <Command.Group
              heading="Workspace"
              className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1 px-3 pt-4"
            >
              <Command.Item
                onSelect={() => {
                  useStore.getState().setTerminalOpen(!useStore.getState().isTerminalOpen);
                  setOpen(false);
                }}
                className="flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors"
              >
                Toggle Terminal Panel
              </Command.Item>
              <Command.Item
                onSelect={() => {
                  // @ts-ignore
                  if (window.sendNexus) window.sendNexus('ClearChat', {});
                  setOpen(false);
                }}
                className="flex items-center px-3 py-3 mt-1 rounded-lg cursor-pointer hover:bg-accent/20 aria-selected:bg-accent/20 text-foreground text-sm transition-colors"
              >
                Clear Chat History
              </Command.Item>
            </Command.Group>
          </Command.List>
        </Command>
      </div>
    </div>
  );
}
