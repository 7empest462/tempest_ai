import './style.css'
import * as wasm from './pkg/tempest_wasm.js'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

declare var hljs: any;

const extensionMap: Record<string, string> = {
  'rs': 'rust', 'ts': 'typescript', 'js': 'javascript', 'sh': 'bash',
  'toml': 'toml', 'md': 'markdown', 'json': 'json', 'html': 'xml',
  'css': 'css', 'py': 'python', 'yml': 'yaml', 'yaml': 'yaml',
  'zig': 'zig', 'nix': 'nix', 'c': 'c', 'cpp': 'cpp', 'h': 'cpp'
};

async function startApp() {
  // 1. Initialize WASM
  let dashboard: any = null;
  try {
    dashboard = await wasm.initialize_dashboard('vortex-canvas');
    console.log("🌪️ WASM Dashboard Online");
    window.addEventListener('resize', () => {
      const canvas = document.getElementById('vortex-canvas') as HTMLCanvasElement;
      if (canvas) {
        canvas.width = window.innerWidth;
        canvas.height = window.innerHeight;
        dashboard.resize(window.innerWidth, window.innerHeight);
      }
    });
  } catch (e) {
    console.error("❌ Failed to load WASM:", e);
  }

  // 2. UI Elements
  const chatInput = document.getElementById('chat-input') as HTMLTextAreaElement;
  const sendBtn = document.getElementById('send-btn');
  const chatMessages = document.getElementById('chat-messages');
  const cpuMetric = document.getElementById('cpu-metric');
  const gpuMetric = document.getElementById('gpu-metric');
  const ramMetric = document.getElementById('ram-metric');
  const fileExplorer = document.getElementById('file-explorer');
  const engineStatus = document.getElementById('engine-status');
  const editorContainer = document.getElementById('editor-container');
  const editorFilename = document.getElementById('editor-filename');
  const codeDisplay = document.getElementById('code-display')?.querySelector('code');
  const closeEditor = document.getElementById('close-editor');
  const editBtn = document.getElementById('edit-btn');
  const backBtn = document.getElementById('back-btn');
  const pathLabel = document.getElementById('current-path-label');

  // Panel elements
  const terminalPanel = document.getElementById('terminal-panel');
  const searchPanel = document.getElementById('search-panel');
  const setupPanel = document.getElementById('setup-panel');
  const navSearch = document.getElementById('nav-search');
  const navTerminal = document.getElementById('nav-terminal');
  const navSettings = document.getElementById('nav-settings');

  // 3. State
  let currentPath = '.';
  let currentOpenFile: string | null = null;
  let socket: WebSocket | null = null;
  let isEditing = false;
  let currentFileExt = '';
  let terminalSpawned = false;

  // 4. xterm.js Terminal
  const term = new Terminal({
    fontFamily: "'JetBrains Mono', monospace",
    fontSize: 13,
    theme: {
      background: 'transparent',
      foreground: '#d0d0e0',
      cursor: '#00f2ff',
      cursorAccent: '#05050a',
      selectionBackground: 'rgba(0, 242, 255, 0.2)',
    },
    cursorBlink: true,
    allowProposedApi: true,
  });
  const fitAddon = new FitAddon();
  term.loadAddon(fitAddon);

  // 5. WebSocket Connection
  const connect = () => {
    socket = new WebSocket('ws://localhost:8080/ws');
    socket.onopen = () => {
      console.log("📡 [NEXUS]: Connection established.");
      appendMessage('system', "📡 [NEXUS]: Neural link synchronized.");
      fetchExplorer('.');
    };
    socket.onclose = () => {
      console.log("❌ [NEXUS]: Connection lost. Retrying...");
      setTimeout(connect, 2000);
    };
    socket.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      handleNexusMessage(msg);
    };
  };

  const sendNexus = (type: string, payload: any) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type, payload }));
    }
  };

  const highlightCode = (content: string, ext: string) => {
    if (!codeDisplay || typeof hljs === 'undefined') return;
    const lang = extensionMap[ext.toLowerCase()];
    try {
      if (lang && hljs.getLanguage(lang)) {
        const result = hljs.highlight(content, { language: lang });
        codeDisplay.innerHTML = result.value;
        codeDisplay.className = `hljs language-${lang}`;
      } else {
        const result = hljs.highlightAuto(content);
        codeDisplay.innerHTML = result.value;
        codeDisplay.className = `hljs ${result.language || ''}`;
      }
    } catch (e) {
      codeDisplay.innerText = content;
      codeDisplay.className = 'hljs plaintext';
    }
  };

  const handleNexusMessage = (msg: any) => {
    switch (msg.type) {
      case 'Token':
        updateLastMessage(msg.payload.text);
        break;
      case 'Done':
        break;
      case 'Telemetry':
        if (cpuMetric) cpuMetric.innerText = `CPU: ${msg.payload.cpu.toFixed(1)}%`;
        if (gpuMetric) gpuMetric.innerText = `GPU: ${msg.payload.gpu.toFixed(1)}%`;
        if (ramMetric) ramMetric.innerText = `RAM: ${msg.payload.ram}`;
        break;
      case 'FileTree':
        currentPath = msg.payload.current_path;
        if (pathLabel) pathLabel.innerText = currentPath;
        renderExplorer(msg.payload.items);
        break;
      case 'FileContent':
        if (editorContainer) editorContainer.classList.remove('hidden');
        if (codeDisplay) {
          codeDisplay.innerText = msg.payload.content;
          highlightCode(msg.payload.content, currentFileExt);
          isEditing = false;
          if (editBtn) editBtn.innerText = 'EDIT';
          codeDisplay.contentEditable = 'false';
        }
        break;
      case 'TerminalOutput':
        term.write(msg.payload.data);
        break;
      case 'SearchResults':
        renderSearchResults(msg.payload.matches);
        break;
      case 'BackendInfo': {
        const radio = document.querySelector(`input[name="backend"][value="${msg.payload.backend}"]`) as HTMLInputElement;
        if (radio) {
          radio.checked = true;
          if (engineStatus) {
            const names: Record<string, string> = { mlx: 'MLX (Metal)', ollama: 'Ollama', bridge: 'AI Bridge', lmstudio: 'LM Studio' };
            engineStatus.innerText = `Engine: ${names[msg.payload.backend] || msg.payload.backend}`;
          }
        }
        break;
      }
      case 'Error':
        appendMessage('system', `❌ [ERROR]: ${msg.payload.message}`);
        break;
    }
  };

  connect();

  // 6. Chat Logic
  let lastAiMessage: HTMLElement | null = null;

  const appendMessage = (role: 'user' | 'ai' | 'system', content: string) => {
    if (!chatMessages) return;
    const msgDiv = document.createElement('div');
    msgDiv.className = `message ${role}`;
    msgDiv.innerHTML = `<div class="content">${content}</div>`;
    chatMessages.appendChild(msgDiv);
    chatMessages.scrollTop = chatMessages.scrollHeight;
    if (role === 'ai') lastAiMessage = msgDiv.querySelector('.content');
    return msgDiv;
  };

  const updateLastMessage = (text: string) => {
    if (lastAiMessage) {
      lastAiMessage.innerText += text;
      if (chatMessages) chatMessages.scrollTop = chatMessages.scrollHeight;
    } else {
      appendMessage('ai', text);
    }
  };

  const handleSend = () => {
    const text = chatInput.value.trim();
    if (!text) return;
    appendMessage('user', text);
    chatInput.value = '';
    lastAiMessage = null;
    sendNexus('Chat', { message: text });
  };

  sendBtn?.addEventListener('click', handleSend);
  chatInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  });

  // 7. File Explorer Logic
  const fetchExplorer = (path: string) => {
    sendNexus('ListFiles', { path });
  };

  const renderExplorer = (items: any[]) => {
    if (!fileExplorer) return;
    fileExplorer.innerHTML = '';
    items.sort((a, b) => (b.is_dir ? 1 : 0) - (a.is_dir ? 1 : 0)).forEach(item => {
      const div = document.createElement('div');
      div.className = `explorer-item ${item.is_dir ? 'folder' : 'file'}`;
      div.innerText = item.name;
      div.onclick = () => {
        const fullPath = `${currentPath}/${item.name}`;
        if (!item.is_dir) {
          if (editorFilename) editorFilename.innerText = item.name;
          currentFileExt = item.name.split('.').pop() || '';
          currentOpenFile = fullPath;
          sendNexus('ReadFile', { path: fullPath });
        } else {
          fetchExplorer(fullPath);
        }
      };
      fileExplorer.appendChild(div);
    });
  };

  backBtn?.addEventListener('click', () => {
    fetchExplorer(`${currentPath}/..`);
  });

  closeEditor?.addEventListener('click', () => {
    if (editorContainer) editorContainer.classList.add('hidden');
    currentOpenFile = null;
  });

  editBtn?.addEventListener('click', () => {
    if (!codeDisplay) return;
    isEditing = !isEditing;
    if (isEditing) {
      codeDisplay.contentEditable = 'true';
      editBtn.innerText = 'SAVE';
      codeDisplay.focus();
      codeDisplay.className = 'hljs plaintext';
      codeDisplay.innerText = codeDisplay.innerText;
    } else {
      const content = codeDisplay.innerText;
      if (currentOpenFile) {
        sendNexus('WriteFile', { path: currentOpenFile, content });
        appendMessage('system', `💾 [NEXUS]: Successfully saved ${editorFilename?.innerText} to disk.`);
      }
      codeDisplay.contentEditable = 'false';
      editBtn.innerText = 'EDIT';
      highlightCode(content, currentFileExt);
    }
  });

  // 8. Terminal Panel Logic
  const openTerminal = () => {
    terminalPanel?.classList.remove('panel-hidden');
    navTerminal?.classList.add('active');

    if (!terminalSpawned) {
      const container = document.getElementById('terminal-container');
      if (container) {
        term.open(container);
        fitAddon.fit();
        term.onData((data: string) => {
          sendNexus('TerminalInput', { data });
        });
        term.onResize(({ cols, rows }) => {
          sendNexus('TerminalResize', { cols, rows });
        });
        sendNexus('TerminalSpawn', {});
        terminalSpawned = true;
      }
    } else {
      // Re-fit when reopening
      setTimeout(() => fitAddon.fit(), 50);
    }
  };

  navTerminal?.addEventListener('click', () => {
    const isVisible = !terminalPanel?.classList.contains('panel-hidden');
    if (isVisible) {
      terminalPanel?.classList.add('panel-hidden');
      navTerminal?.classList.remove('active');
    } else {
      openTerminal();
    }
  });

  document.getElementById('close-terminal')?.addEventListener('click', () => {
    terminalPanel?.classList.add('panel-hidden');
    navTerminal?.classList.remove('active');
  });

  // Auto-fit terminal on window resize
  window.addEventListener('resize', () => {
    if (terminalSpawned && !terminalPanel?.classList.contains('panel-hidden')) {
      fitAddon.fit();
    }
  });

  // 9. Search Panel Logic
  const searchInput = document.getElementById('search-input') as HTMLInputElement;
  const searchGoBtn = document.getElementById('search-go-btn');
  const searchResults = document.getElementById('search-results');

  const doSearch = () => {
    const query = searchInput?.value.trim();
    if (!query) return;
    if (searchResults) searchResults.innerHTML = '<div style="color: var(--text-secondary); padding: 12px; font-size: 0.8rem;">Searching...</div>';
    sendNexus('SearchFiles', { query, path: '.' });
  };

  const renderSearchResults = (matches: any[]) => {
    if (!searchResults) return;
    if (matches.length === 0) {
      searchResults.innerHTML = '<div style="color: var(--text-secondary); padding: 12px; font-size: 0.8rem;">No results found.</div>';
      return;
    }
    searchResults.innerHTML = '';
    matches.forEach(match => {
      const div = document.createElement('div');
      div.className = 'search-result';
      div.innerHTML = `
        <span class="search-result-file">${match.file}</span>
        <span class="search-result-line">:${match.line}</span>
        <div class="search-result-content">${escapeHtml(match.content)}</div>
      `;
      div.onclick = () => {
        // Open the file in the editor
        const ext = match.file.split('.').pop() || '';
        currentFileExt = ext;
        currentOpenFile = match.file;
        if (editorFilename) editorFilename.innerText = match.file.split('/').pop() || match.file;
        sendNexus('ReadFile', { path: match.file });
        // Close search panel
        searchPanel?.classList.add('panel-hidden');
        navSearch?.classList.remove('active');
      };
      searchResults.appendChild(div);
    });
  };

  const escapeHtml = (str: string) => {
    const div = document.createElement('div');
    div.innerText = str;
    return div.innerHTML;
  };

  navSearch?.addEventListener('click', () => {
    const isVisible = !searchPanel?.classList.contains('panel-hidden');
    if (isVisible) {
      searchPanel?.classList.add('panel-hidden');
      navSearch?.classList.remove('active');
    } else {
      searchPanel?.classList.remove('panel-hidden');
      navSearch?.classList.add('active');
      searchInput?.focus();
    }
  });

  searchGoBtn?.addEventListener('click', doSearch);
  searchInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') doSearch();
  });

  document.getElementById('close-search')?.addEventListener('click', () => {
    searchPanel?.classList.add('panel-hidden');
    navSearch?.classList.remove('active');
  });

  // 10. Setup Panel Logic
  const tempSlider = document.getElementById('temp-slider') as HTMLInputElement;
  const tempValue = document.getElementById('temp-value');
  const tokensSlider = document.getElementById('tokens-slider') as HTMLInputElement;
  const tokensValue = document.getElementById('tokens-value');

  tempSlider?.addEventListener('input', () => {
    const val = (parseInt(tempSlider.value) / 100).toFixed(2);
    if (tempValue) tempValue.innerText = val;
  });

  tokensSlider?.addEventListener('input', () => {
    if (tokensValue) tokensValue.innerText = tokensSlider.value;
  });

  // Background options
  document.querySelectorAll('.bg-option').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.bg-option').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      const bg = (btn as HTMLElement).dataset.bg;
      const vortexContainer = document.getElementById('vortex-container');
      if (bg === 'dark' && vortexContainer) {
        vortexContainer.style.display = 'none';
      } else if (vortexContainer) {
        vortexContainer.style.display = 'block';
      }
      appendMessage('system', `🎨 [SETTINGS]: Background changed to ${bg}.`);
    });
  });

  navSettings?.addEventListener('click', () => {
    const isVisible = !setupPanel?.classList.contains('panel-hidden');
    if (isVisible) {
      setupPanel?.classList.add('panel-hidden');
      navSettings?.classList.remove('active');
    } else {
      setupPanel?.classList.remove('panel-hidden');
      navSettings?.classList.add('active');
    }
  });

  document.getElementById('close-setup')?.addEventListener('click', () => {
    setupPanel?.classList.add('panel-hidden');
    navSettings?.classList.remove('active');
  });

  // 11. Backend Toggle
  const backendRadios = document.querySelectorAll('input[name="backend"]');
  backendRadios.forEach(radio => {
    radio.addEventListener('change', (e) => {
      const target = e.target as HTMLInputElement;
      if (engineStatus) engineStatus.innerText = `Engine: ${target.value.toUpperCase()}`;
      appendMessage('system', `📡 [BACKEND]: Switching to ${target.value.toUpperCase()}...`);
    });
  });
}

startApp();
