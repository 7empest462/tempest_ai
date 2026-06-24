import Editor, { loader } from '@monaco-editor/react';
import { useStore } from '../store';

// Configure Monaco to load from local assets served by Vite
loader.config({
  paths: { vs: '/monaco-editor/min/vs' },
});

export function CodeEditor() {
  const { activeFile, setEditorFocused, isFileEditable } = useStore();

  if (!activeFile) {
    return (
      <div className="flex-1 flex items-center justify-center bg-black/40 text-muted-foreground text-sm font-mono p-4">
        <div className="text-center">
          <p className="mb-2">No active file</p>
          <p className="opacity-50">Select a file from the explorer to view it here.</p>
        </div>
      </div>
    );
  }

  // Determine language by extension
  const extMap: Record<string, string> = {
    rs: 'rust',
    zig: 'zig',
    ts: 'typescript',
    tsx: 'typescript',
    js: 'javascript',
    jsx: 'javascript',
    sh: 'shell',
    bash: 'shell',
    zsh: 'shell',
    fish: 'shell',
    nix: 'nix',
    toml: 'toml',
    lock: 'toml',
    md: 'markdown',
    markdown: 'markdown',
    json: 'json',
    html: 'html',
    css: 'css',
    py: 'python',
    yml: 'yaml',
    yaml: 'yaml',
    c: 'c',
    cpp: 'cpp',
    h: 'cpp',
    cmake: 'cmake',
    sshconfig: 'ini',
    'ssh-config': 'ini',
    ssh: 'ini',
    asm: 'assembly',
    s: 'assembly',
    txt: 'plaintext',
  };

  const language = extMap[activeFile.ext.toLowerCase()] || 'plaintext';

  return (
    <div
      className="flex-1 w-full h-full"
      onFocus={() => setEditorFocused(true)}
      onBlur={() => setEditorFocused(false)}
    >
      <Editor
        height="100%"
        language={language}
        theme="vs-dark"
        value={activeFile.content}
        onChange={(val) => useStore.getState().updateActiveFileContent(val || '')}
        beforeMount={(monaco) => {
          if (!monaco.languages.getLanguages().some((lang: any) => lang.id === 'toml')) {
            monaco.languages.register({ id: 'toml' });
            monaco.languages.setMonarchTokensProvider('toml', {
              defaultToken: '',
              tokenPostfix: '.toml',
              keywords: ['true', 'false'],
              tokenizer: {
                root: [
                  [/\[[^\]]+\]/, 'metatag'],
                  [/[a-zA-Z0-9_-]+(?=\s*=)/, 'attribute.name'],
                  [/(=)/, 'operator'],
                  [
                    /[a-zA-Z_]\w*/,
                    {
                      cases: {
                        '@keywords': 'keyword',
                        '@default': 'identifier',
                      },
                    },
                  ],
                  [/#.*$/, 'comment'],
                  [/\d+(\.\d+)?/, 'number'],
                  [/"([^"\\]|\\.)*"/, 'string'],
                  [/'([^'\\]|\\.)*'/, 'string'],
                ],
              },
            });
          }
        }}
        options={{
          readOnly: !isFileEditable,
          minimap: { enabled: false },
          fontSize: 13,
          fontFamily: '"JetBrains Mono", monospace',
          scrollBeyondLastLine: false,
          smoothScrolling: true,
          cursorBlinking: 'smooth',
        }}
        loading={
          <div className="text-accent animate-pulse font-mono text-sm p-4">Loading editor...</div>
        }
      />
    </div>
  );
}
