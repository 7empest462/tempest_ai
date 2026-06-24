import { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';

export function TerminalPanel() {
  const terminalRef = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fitAddon = useRef(new FitAddon());

  useEffect(() => {
    if (!terminalRef.current || term.current) return;

    term.current = new Terminal({
      theme: {
        background: 'transparent',
        foreground: '#a0a0c0',
        cursor: '#00f2ff',
        cursorAccent: '#000000',
        selectionBackground: 'rgba(0, 242, 255, 0.3)',
      },
      fontFamily: '"JetBrains Mono", monospace',
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      convertEol: true,
    });

    term.current.loadAddon(fitAddon.current);
    term.current.open(terminalRef.current);
    fitAddon.current.fit();

    term.current.writeln('\x1b[1;36m🌪️ Terminal Subsystem Online.\x1b[0m');

    // @ts-ignore
    if (window.sendNexus) {
      // @ts-ignore
      window.sendNexus('TerminalSpawn', {});
    }

    const onDataDisposable = term.current.onData((data: string) => {
      // @ts-ignore
      if (window.sendNexus) window.sendNexus('TerminalInput', { data });
    });

    const onResizeDisposable = term.current.onResize(({ cols, rows }) => {
      // @ts-ignore
      if (window.sendNexus) window.sendNexus('TerminalResize', { cols, rows });
    });

    const onTerminalOutput = (e: Event) => {
      const customEvent = e as CustomEvent;
      term.current?.write(customEvent.detail);
    };
    window.addEventListener('terminal-output', onTerminalOutput);

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.current.fit();
        } catch {}
      });
    });

    resizeObserver.observe(terminalRef.current);

    return () => {
      onDataDisposable.dispose();
      onResizeDisposable.dispose();
      window.removeEventListener('terminal-output', onTerminalOutput);
      resizeObserver.disconnect();
      term.current?.dispose();
      term.current = null;
    };
  }, []);

  return <div ref={terminalRef} className="h-full w-full p-2" />;
}
