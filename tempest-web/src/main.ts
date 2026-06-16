import './style.css'
import * as wasm from './pkg/tempest_wasm.js'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

declare var hljs: any;

const extensionMap: Record<string, string> = {
  'rs': 'rust', 'ts': 'typescript', 'tsx': 'typescript', 'js': 'javascript', 'jsx': 'javascript', 'sh': 'bash',
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
  const stopBtn = document.getElementById('stop-btn');
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
  
  // Mission Control Elements
  const stepThinking = document.getElementById('step-thinking');
  const stepPlanning = document.getElementById('step-planning');
  const stepExecuting = document.getElementById('step-executing');

  const updateStepper = (state: string) => {
    [stepThinking, stepPlanning, stepExecuting].forEach(el => el?.classList.remove('active', 'pulse'));
    if (state === 'Thinking') {
      stepThinking?.classList.add('active', 'pulse');
    }
    if (state === 'Planning' || state === 'PendingTools') {
      stepPlanning?.classList.add('active', 'pulse');
    }
    if (state === 'Executing' || state === 'ExecutingTools') {
      stepExecuting?.classList.add('active', 'pulse');
    }
  };
  const activeToolsList = document.getElementById('active-tools-list');
  
  // Safe Mode Elements
  const safemodeModal = document.getElementById('safemode-modal');
  const safemodeRationale = document.getElementById('safemode-rationale');
  const safemodeDiff = document.getElementById('safemode-diff-container');
  const safemodeApprove = document.getElementById('safemode-approve');
  const safemodeReject = document.getElementById('safemode-reject');

  // Task Elements
  const activeGoalBox = document.getElementById('active-goal-box');
  const activeGoalText = document.getElementById('active-goal-text');

  // Reasoning Elements
  const reasoningMonitor = document.getElementById('reasoning-monitor');
  const reasoningText = document.getElementById('reasoning-text');

  // Panel elements
  const terminalPanel = document.getElementById('terminal-panel');
  const searchPanel = document.getElementById('search-panel');
  const setupPanel = document.getElementById('setup-panel');
  const navSearch = document.getElementById('nav-search');
  const navTerminal = document.getElementById('nav-terminal');
  const navSettings = document.getElementById('nav-settings');

  // Footer metrics
  const tpsMetric = document.getElementById('tps-metric');
  const ctxMetric = document.getElementById('ctx-metric');

  // Dropdown elements
  const ddPlanner = document.getElementById('dd-planner');
  const ddExecutor = document.getElementById('dd-executor');
  const ddVerifier = document.getElementById('dd-verifier');
  const ddBackend = document.getElementById('dd-backend');
  const ddDuration = document.getElementById('dd-duration');
  const ddMessages = document.getElementById('dd-messages');
  const ddToolCalls = document.getElementById('dd-tool-calls');
  const ddTokens = document.getElementById('dd-tokens');
  const ddPeakTps = document.getElementById('dd-peak-tps');

  // 3. State
  let currentPath = '.';
  let currentOpenFile: string | null = null;
  let socket: WebSocket | null = null;
  let isEditing = false;
  let currentFileExt = '';
  let terminalSpawned = false;
  // (streaming state tracked by button visibility)
  let inLeakedThink = false;
  let streamAccum = '';

  // Session stats
  const sessionStart = Date.now();
  let sessionMessages = 0;
  let sessionToolCalls = 0;
  let sessionTokens = 0;
  let peakTps = 0;
  let lastTps = 0;
  let lastCtxUsed = 0;
  let lastCtxTotal = 0;

  // Duration ticker
  setInterval(() => {
    const elapsed = Math.floor((Date.now() - sessionStart) / 1000);
    const mins = Math.floor(elapsed / 60).toString().padStart(2, '0');
    const secs = (elapsed % 60).toString().padStart(2, '0');
    if (ddDuration) ddDuration.innerText = `${mins}:${secs}`;
  }, 1000);

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

  // 5. Send/Stop Toggle
  const setStreamingState = (streaming: boolean) => {
    if (streaming) {
      sendBtn?.classList.add('hidden');
      stopBtn?.classList.remove('hidden');
    } else {
      stopBtn?.classList.add('hidden');
      sendBtn?.classList.remove('hidden');
    }
  };

  // 6. WebSocket Connection
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

  const resetTurnState = () => {
    lastAiMessage = null;
    inLeakedThink = false;
    streamAccum = '';
    if (reasoningText) reasoningText.innerText = '';
    if (reasoningMonitor) reasoningMonitor.classList.add('hidden');
  };

  const handleNexusMessage = (msg: any) => {
    switch (msg.type) {
      case 'Token':
        updateLastMessage(msg.payload.text);
        break;
      case 'Done':
        setStreamingState(false);
        // Reset TPS to idle
        if (tpsMetric) {
          tpsMetric.innerText = 'TPS: idle';
          tpsMetric.classList.remove('live');
        }
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
        const names: Record<string, string> = { mlx: 'MLX (Metal)', ollama: 'Ollama', bridge: 'AI Bridge', lmstudio: 'LM Studio', kalosm: 'Kalosm (Native GPU)' };
        if (engineStatus) {
          engineStatus.innerText = `Engine: ${names[msg.payload.backend] || msg.payload.backend}`;
        }
        // Populate model dropdown
        if (ddPlanner) ddPlanner.innerText = msg.payload.planner || '--';
        if (ddExecutor) ddExecutor.innerText = msg.payload.executor || '--';
        if (ddVerifier) ddVerifier.innerText = msg.payload.verifier || '--';
        if (ddBackend) ddBackend.innerText = names[msg.payload.backend] || msg.payload.backend;
        break;
      }
      case 'AgentStateChange': {
        const state = msg.payload.state;
        updateStepper(state);
        
        if (state === 'Thinking' || state === 'Planning') {
          resetTurnState();
        }
        
        if (state === 'Done') {
           // Turn off all
           if (activeToolsList) activeToolsList.innerHTML = '<li class="empty-tools">No tools running</li>';
        }
        break;
      }
      case 'ActiveTools': {
        const tools: string[] = msg.payload.tools;
        if (activeToolsList) {
          if (tools.length === 0) {
            activeToolsList.innerHTML = '<li class="empty-tools">No tools running</li>';
          } else {
            activeToolsList.innerHTML = tools.map(t => `<li>${t}</li>`).join('');
          }
        }
        break;
      }
      case 'ToolResult': {
        sessionToolCalls++;
        if (ddToolCalls) ddToolCalls.innerText = sessionToolCalls.toString();
        break;
      }
      case 'SafeModeRequest': {
        if (safemodeModal && safemodeRationale) {
          safemodeRationale.innerText = msg.payload.rationale;
          // Render diff using the new visual renderer
          renderDiff(msg.payload.diff);
          safemodeModal.classList.remove('hidden');
        }
        break;
      }
      case 'TaskUpdate': {
        if (activeGoalBox && activeGoalText) {
          activeGoalText.innerText = msg.payload.task;
          activeGoalBox.classList.remove('hidden');
        }
        break;
      }
      case 'ReasoningToken': {
        updateStepper('Thinking');
        if (reasoningMonitor && reasoningText) {
          reasoningMonitor.classList.remove('hidden');
          reasoningText.innerText += msg.payload.token;
          // Smooth scroll to bottom
          requestAnimationFrame(() => {
            reasoningText.scrollTop = reasoningText.scrollHeight;
          });
        }
        break;
      }
      case 'StreamToken': {
        let token: string = msg.payload.token;

        // Strip DeepSeek prompt template artifacts (individual token)
        token = token.replace(/<｜begin of sentence｜>/g, '');
        token = token.replace(/<｜end of sentence｜>/g, '');
        token = token.replace(/<\|begin of sentence\|>/g, '');
        token = token.replace(/<\|end of sentence\|>/g, '');

        // Handle leaked <think> blocks
        if (token.includes('<think>')) {
          inLeakedThink = true;
          token = token.replace(/<think>/g, '');
        }
        if (token.includes('</think>')) {
          inLeakedThink = false;
          token = token.replace(/<\/think>/g, '');
        }

        // If inside a leaked think block, redirect to reasoning monitor
        if (inLeakedThink) {
          if (reasoningMonitor && reasoningText) {
            reasoningMonitor.classList.remove('hidden');
            reasoningText.innerText += token;
          }
          break;
        }

        // Buffer tokens to detect multi-token leaked patterns
        streamAccum += token;

        // If accumulator contains a leaked prompt echo, buffer until newline
        if (streamAccum.includes('Human:') || streamAccum.includes('[EDITOR]') || streamAccum.includes('Assistant:')) {
          if (!streamAccum.includes('\n')) {
            break;
          }
          streamAccum = streamAccum
            .split('\n')
            .filter(line => {
              const t = line.trim();
              return !t.startsWith('Human:') &&
                     !t.startsWith('[EDITOR]') &&
                     !t.startsWith('Assistant:') &&
                     t !== '';
            })
            .join('\n');
        }

        // Check if buffer ends with a partial pattern
        const partials = ['Human', 'Human:', '[EDITOR', '[EDITOR]', 'Assistant', 'Assistant:'];
        if (partials.some(p => streamAccum.endsWith(p)) && streamAccum.length < 200) {
          break;
        }

        // Flush accumulator
        token = streamAccum;
        
        // Strip again after buffering to catch split/partial template tokens!
        token = token.replace(/<｜begin of sentence｜>/g, '');
        token = token.replace(/<｜end of sentence｜>/g, '');
        token = token.replace(/<\|begin of sentence\|>/g, '');
        token = token.replace(/<\|end of sentence\|>/g, '');
        
        streamAccum = '';

        if (token.trim().length === 0) break;

        updateStepper('Executing');
        updateLastMessage(token);
        sessionTokens++;
        if (ddTokens) ddTokens.innerText = sessionTokens.toString();
        break;
      }
      case 'InferenceMetrics': {
        if (msg.payload.tps != null) {
          lastTps = msg.payload.tps;
          if (tpsMetric) {
            tpsMetric.innerText = `TPS: ${lastTps}`;
            tpsMetric.classList.add('live');
          }
          // Track peak
          if (lastTps > peakTps) {
            peakTps = lastTps;
            if (ddPeakTps) ddPeakTps.innerText = `${peakTps} t/s`;
          }
        }
        if (msg.payload.ctx_used != null && msg.payload.ctx_total != null) {
          lastCtxUsed = msg.payload.ctx_used;
          lastCtxTotal = msg.payload.ctx_total;
          const totalK = (lastCtxTotal / 1024).toFixed(0);
          const usedK = (lastCtxUsed / 1024).toFixed(1);
          if (ctxMetric) {
            ctxMetric.innerText = `CTX: ${usedK}k/${totalK}k`;
            ctxMetric.classList.add('live');
          }
        }
        break;
      }
      case 'SentinelLog': {
        // Count sentinel events in session stats
        appendMessage('system', `🛡️ [${msg.payload.sentinel}]: ${msg.payload.message}`);
        break;
      }
      case 'Error':
        appendMessage('system', `❌ [ERROR]: ${msg.payload.message}`);
        break;
    }
  };

  connect();

  // 7. Chat Logic
  let lastAiMessage: HTMLElement | null = null;

  const truncateAtCompleteJson = (text: string): string => {
    let depth = 0;
    let inString = false;
    let escape = false;
    let started = false;
    const startIdx = text.indexOf('{');
    if (startIdx === -1) return text;
    
    for (let i = startIdx; i < text.length; i++) {
      const c = text[i];
      if (escape) {
        escape = false;
        continue;
      }
      if (c === '\\') {
        escape = true;
        continue;
      }
      if (c === '"') {
        inString = !inString;
        continue;
      }
      if (!inString) {
        if (c === '{') {
          depth++;
          started = true;
        } else if (c === '}') {
          depth--;
          if (started && depth === 0) {
            const jsonPart = text.substring(startIdx, i + 1);
            const lower = jsonPart.toLowerCase();
            if (lower.includes('"tool"') || lower.includes('"name"') || lower.includes('"is_valid"')) {
              return text.substring(0, i + 1);
            }
          }
        }
      }
    }
    return text;
  };

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
      const cleaned = truncateAtCompleteJson(lastAiMessage.innerText);
      if (cleaned !== lastAiMessage.innerText) {
        lastAiMessage.innerText = cleaned;
      }
      if (chatMessages) chatMessages.scrollTop = chatMessages.scrollHeight;
    } else {
      appendMessage('ai', text);
    }
  };

  const handleSend = () => {
    const text = (chatInput as HTMLTextAreaElement).value.trim();
    if (!text) return;

    // Clear old reasoning
    if (reasoningText) reasoningText.innerText = '';
    if (reasoningMonitor) reasoningMonitor.classList.add('hidden');
    inLeakedThink = false;
    streamAccum = '';
    appendMessage('user', text);
    chatInput.value = '';
    lastAiMessage = null;
    sessionMessages++;
    if (ddMessages) ddMessages.innerText = sessionMessages.toString();
    sendNexus('Chat', { message: text, editor_context: currentOpenFile || undefined });
    setStreamingState(true);
  };

  const handleStop = () => {
    sendNexus('StopStream', {});
    setStreamingState(false);
    appendMessage('system', '⏹️ [SYSTEM]: Stream aborted by user.');
  };

  sendBtn?.addEventListener('click', handleSend);
  stopBtn?.addEventListener('click', handleStop);
  chatInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  });

  // 8. Dropdown Logic
  const modelDropdownBtn = document.getElementById('model-dropdown-btn');
  const modelDropdown = document.getElementById('model-dropdown');
  const sessionDropdownBtn = document.getElementById('session-dropdown-btn');
  const sessionDropdown = document.getElementById('session-dropdown');

  const toggleDropdown = (panel: HTMLElement | null) => {
    if (!panel) return;
    // Close all other dropdowns first
    document.querySelectorAll('.dropdown-panel').forEach(p => {
      if (p !== panel) p.classList.remove('open');
    });
    panel.classList.toggle('open');
  };

  modelDropdownBtn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleDropdown(modelDropdown);
  });

  sessionDropdownBtn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleDropdown(sessionDropdown);
  });

  // Close dropdowns on outside click
  document.addEventListener('click', () => {
    document.querySelectorAll('.dropdown-panel').forEach(p => p.classList.remove('open'));
  });

  // Prevent clicks inside dropdowns from closing them
  document.querySelectorAll('.dropdown-panel').forEach(panel => {
    panel.addEventListener('click', (e) => e.stopPropagation());
  });

  // 9. File Explorer Logic
  let selectedItem: { name: string, is_dir: boolean, path: string } | null = null;

  const fetchExplorer = (path: string) => {
    selectedItem = null;
    sendNexus('ListFiles', { path });
  };

  const renderExplorer = (items: any[]) => {
    if (!fileExplorer) return;
    fileExplorer.innerHTML = '';
    items.sort((a, b) => (b.is_dir ? 1 : 0) - (a.is_dir ? 1 : 0)).forEach(item => {
      const div = document.createElement('div');
      div.className = `explorer-item ${item.is_dir ? 'folder' : 'file'}`;
      div.innerText = item.name;
      const fullPath = `${currentPath}/${item.name}`;

      // Single click = select
      div.onclick = (e) => {
        e.stopPropagation();
        document.querySelectorAll('.explorer-item').forEach(el => el.classList.remove('selected'));
        div.classList.add('selected');
        selectedItem = { name: item.name, is_dir: item.is_dir, path: fullPath };
      };

      // Double click = open file or navigate folder
      div.ondblclick = () => {
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

  // Deselect on clicking empty explorer area
  fileExplorer?.addEventListener('click', () => {
    document.querySelectorAll('.explorer-item').forEach(el => el.classList.remove('selected'));
    selectedItem = null;
  });

  // New File
  document.getElementById('new-file-btn')?.addEventListener('click', () => {
    const name = prompt('New file name:');
    if (!name) return;
    const path = `${currentPath}/${name}`;
    sendNexus('CreateFile', { path });
    appendMessage('system', `📄 [NEXUS]: Created file ${name}`);
    setTimeout(() => fetchExplorer(currentPath), 200);
  });

  // New Folder
  document.getElementById('new-folder-btn')?.addEventListener('click', () => {
    const name = prompt('New folder name:');
    if (!name) return;
    const path = `${currentPath}/${name}`;
    sendNexus('CreateFolder', { path });
    appendMessage('system', `📁 [NEXUS]: Created folder ${name}`);
    setTimeout(() => fetchExplorer(currentPath), 200);
  });

  // Rename
  document.getElementById('rename-btn')?.addEventListener('click', () => {
    if (!selectedItem) {
      appendMessage('system', '⚠️ [NEXUS]: Select a file or folder first.');
      return;
    }
    const newName = prompt(`Rename "${selectedItem.name}" to:`, selectedItem.name);
    if (!newName || newName === selectedItem.name) return;
    const newPath = `${currentPath}/${newName}`;
    sendNexus('RenameItem', { old_path: selectedItem.path, new_path: newPath });
    appendMessage('system', `✏️ [NEXUS]: Renamed ${selectedItem.name} → ${newName}`);
    selectedItem = null;
    setTimeout(() => fetchExplorer(currentPath), 200);
  });

  // Delete
  document.getElementById('delete-btn')?.addEventListener('click', () => {
    if (!selectedItem) {
      appendMessage('system', '⚠️ [NEXUS]: Select a file or folder first.');
      return;
    }
    const ok = confirm(`Delete "${selectedItem.name}"? This cannot be undone.`);
    if (!ok) return;
    sendNexus('DeleteItem', { path: selectedItem.path });
    appendMessage('system', `🗑️ [NEXUS]: Deleted ${selectedItem.name}`);
    if (currentOpenFile === selectedItem.path) {
      if (editorContainer) editorContainer.classList.add('hidden');
      currentOpenFile = null;
    }
    selectedItem = null;
    setTimeout(() => fetchExplorer(currentPath), 200);
  });

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

  // 10. Terminal Panel Logic
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

  // 11. Search Panel Logic
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

  // 12. Setup Panel Logic
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
    setupPanel?.classList.add('hidden');
    navSettings?.classList.remove('active');
  });

  // 13. Safe Mode Approval
  safemodeApprove?.addEventListener('click', () => {
    sendNexus('SafeModeApprove', {});
    safemodeModal?.classList.add('hidden');
  });

  safemodeReject?.addEventListener('click', () => {
    sendNexus('SafeModeReject', {});
    safemodeModal?.classList.add('hidden');
  });

  // Edit button toggles contentEditable on added lines
  const safemodeEdit = document.getElementById('safemode-edit');
  safemodeEdit?.addEventListener('click', () => {
    const container = document.getElementById('safemode-diff-container');
    if (!container) return;
    container.classList.toggle('diff-editable');
    const isEditable = container.classList.contains('diff-editable');
    container.querySelectorAll('.diff-line-add .diff-line-content').forEach(el => {
      (el as HTMLElement).contentEditable = isEditable ? 'true' : 'false';
    });
    if (safemodeEdit) {
      safemodeEdit.textContent = isEditable ? '✎ DONE' : '✎ EDIT';
    }
  });

  // Diff view toggle (Unified / Split)
  let currentDiffView: 'unified' | 'split' = 'unified';
  let lastDiffRaw = '';

  document.getElementById('diff-view-unified')?.addEventListener('click', () => {
    currentDiffView = 'unified';
    document.querySelectorAll('.diff-toggle-btn').forEach(b => b.classList.remove('active'));
    document.getElementById('diff-view-unified')?.classList.add('active');
    if (lastDiffRaw) renderDiff(lastDiffRaw);
  });

  document.getElementById('diff-view-split')?.addEventListener('click', () => {
    currentDiffView = 'split';
    document.querySelectorAll('.diff-toggle-btn').forEach(b => b.classList.remove('active'));
    document.getElementById('diff-view-split')?.classList.add('active');
    if (lastDiffRaw) renderDiff(lastDiffRaw);
  });

  // ═══ Visual Diff Renderer ═══
  const renderDiff = (rawDiff: string) => {
    lastDiffRaw = rawDiff;
    if (!safemodeDiff) return;

    const lines = rawDiff.split('\n');
    let addCount = 0;
    let delCount = 0;

    // Detect file name from diff header
    let fileName = '';
    for (const line of lines) {
      if (line.startsWith('+++ ')) {
        fileName = line.slice(4).replace(/^b\//, '');
        break;
      }
      if (line.startsWith('--- ')) {
        fileName = line.slice(4).replace(/^a\//, '');
      }
    }

    // Determine badge type
    const isCreate = lines.some(l => l.startsWith('--- /dev/null'));
    const isDelete = lines.some(l => l.startsWith('+++ /dev/null'));
    const badgeClass = isCreate ? 'created' : isDelete ? 'deleted' : 'modified';
    const badgeText = isCreate ? 'NEW' : isDelete ? 'DEL' : 'MOD';

    // Build file header
    let html = `<div class="diff-file-header">
      <span class="diff-file-icon">📄</span>
      <span class="diff-file-name">${escapeHtml(fileName || 'unknown')}</span>
      <span class="diff-file-badge ${badgeClass}">${badgeText}</span>
    </div>`;

    if (currentDiffView === 'unified') {
      html += renderUnifiedDiff(lines);
    } else {
      html += renderSplitDiff(lines);
    }

    // Count stats
    for (const line of lines) {
      if (line.startsWith('+') && !line.startsWith('+++')) addCount++;
      if (line.startsWith('-') && !line.startsWith('---')) delCount++;
    }

    safemodeDiff.innerHTML = html;

    // Update stats
    const diffStats = document.getElementById('diff-stats');
    if (diffStats) {
      diffStats.innerHTML = `<span class="stat-add">+${addCount}</span> / <span class="stat-del">-${delCount}</span> lines`;
    }
  };

  const renderUnifiedDiff = (lines: string[]): string => {
    let html = '<table class="diff-table"><tbody>';
    let oldLine = 0;
    let newLine = 0;

    for (const line of lines) {
      // Skip diff meta headers
      if (line.startsWith('diff ') || line.startsWith('index ') || 
          line.startsWith('--- ') || line.startsWith('+++ ')) continue;

      // Hunk header
      if (line.startsWith('@@')) {
        const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)/);
        if (match) {
          oldLine = parseInt(match[1]) - 1;
          newLine = parseInt(match[2]) - 1;
        }
        html += `<tr class="diff-hunk-header">
          <td colspan="4">${escapeHtml(line)}</td>
        </tr>`;
        continue;
      }

      if (line.startsWith('+')) {
        newLine++;
        html += `<tr class="diff-line-add">
          <td class="diff-line-num"></td>
          <td class="diff-line-num">${newLine}</td>
          <td class="diff-line-prefix">+</td>
          <td class="diff-line-content">${escapeHtml(line.slice(1))}</td>
        </tr>`;
      } else if (line.startsWith('-')) {
        oldLine++;
        html += `<tr class="diff-line-del">
          <td class="diff-line-num">${oldLine}</td>
          <td class="diff-line-num"></td>
          <td class="diff-line-prefix">−</td>
          <td class="diff-line-content">${escapeHtml(line.slice(1))}</td>
        </tr>`;
      } else {
        oldLine++;
        newLine++;
        const content = line.startsWith(' ') ? line.slice(1) : line;
        html += `<tr class="diff-line-ctx">
          <td class="diff-line-num">${oldLine}</td>
          <td class="diff-line-num">${newLine}</td>
          <td class="diff-line-prefix"> </td>
          <td class="diff-line-content">${escapeHtml(content)}</td>
        </tr>`;
      }
    }

    html += '</tbody></table>';
    return html;
  };

  const renderSplitDiff = (lines: string[]): string => {
    // Collect left (old) and right (new) lines
    const leftLines: { num: number | null; content: string; type: string }[] = [];
    const rightLines: { num: number | null; content: string; type: string }[] = [];
    let oldLine = 0;
    let newLine = 0;

    for (const line of lines) {
      if (line.startsWith('diff ') || line.startsWith('index ') || 
          line.startsWith('--- ') || line.startsWith('+++ ')) continue;

      if (line.startsWith('@@')) {
        const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)/);
        if (match) {
          oldLine = parseInt(match[1]) - 1;
          newLine = parseInt(match[2]) - 1;
        }
        leftLines.push({ num: null, content: line, type: 'hunk' });
        rightLines.push({ num: null, content: line, type: 'hunk' });
        continue;
      }

      if (line.startsWith('-')) {
        oldLine++;
        leftLines.push({ num: oldLine, content: line.slice(1), type: 'del' });
        rightLines.push({ num: null, content: '', type: 'empty' });
      } else if (line.startsWith('+')) {
        newLine++;
        leftLines.push({ num: null, content: '', type: 'empty' });
        rightLines.push({ num: newLine, content: line.slice(1), type: 'add' });
      } else {
        oldLine++;
        newLine++;
        const content = line.startsWith(' ') ? line.slice(1) : line;
        leftLines.push({ num: oldLine, content, type: 'ctx' });
        rightLines.push({ num: newLine, content, type: 'ctx' });
      }
    }

    const renderPane = (paneLines: typeof leftLines): string => {
      let html = '<table class="diff-table"><tbody>';
      for (const ln of paneLines) {
        if (ln.type === 'hunk') {
          html += `<tr class="diff-hunk-header"><td colspan="3">${escapeHtml(ln.content)}</td></tr>`;
          continue;
        }
        const cls = ln.type === 'add' ? 'diff-line-add' : ln.type === 'del' ? 'diff-line-del' : ln.type === 'empty' ? 'diff-line-ctx' : 'diff-line-ctx';
        const prefix = ln.type === 'add' ? '+' : ln.type === 'del' ? '−' : ' ';
        html += `<tr class="${cls}">
          <td class="diff-line-num">${ln.num ?? ''}</td>
          <td class="diff-line-prefix">${ln.type === 'empty' ? '' : prefix}</td>
          <td class="diff-line-content">${escapeHtml(ln.content)}</td>
        </tr>`;
      }
      html += '</tbody></table>';
      return html;
    };

    return `<div class="diff-split-container">
      <div class="diff-split-pane">
        <div class="diff-pane-label">ORIGINAL</div>
        ${renderPane(leftLines)}
      </div>
      <div class="diff-split-pane">
        <div class="diff-pane-label">PROPOSED</div>
        ${renderPane(rightLines)}
      </div>
    </div>`;
  };

  // 14. Resizable Panel Logic
  const setupResize = (handleId: string, getLeft: () => HTMLElement | null, getRight: () => HTMLElement | null, minLeft: number) => {
    const handle = document.getElementById(handleId);
    if (!handle) return;

    let isResizing = false;
    let startX = 0;
    let startLeftWidth = 0;

    handle.addEventListener('mousedown', (e: MouseEvent) => {
      e.preventDefault();
      const left = getLeft();
      const right = getRight();
      if (!left || !right) return;

      isResizing = true;
      startX = e.clientX;
      startLeftWidth = left.getBoundingClientRect().width;
      handle.classList.add('active');
      document.body.classList.add('resizing');
    });

    document.addEventListener('mousemove', (e: MouseEvent) => {
      if (!isResizing) return;
      const left = getLeft();
      const right = getRight();
      if (!left || !right) return;

      const dx = e.clientX - startX;
      // Use flex-basis for better resizing behavior
      const newLeftWidth = Math.max(minLeft, startLeftWidth + dx);
      left.style.width = `${newLeftWidth}px`;
      left.style.flex = `0 0 ${newLeftWidth}px`;
      
      // We don't necessarily need to set the right width if it's the last element
      // but for middle elements like the chat, we might want to.
    });

    document.addEventListener('mouseup', () => {
      if (isResizing) {
        isResizing = false;
        handle.classList.remove('active');
        document.body.classList.remove('resizing');
        // Re-fit terminal if visible
        if (terminalSpawned && !terminalPanel?.classList.contains('panel-hidden')) {
          fitAddon?.fit();
        }
      }
    });
  };

  // Sidebar ↔ Workspace resize
  setupResize(
    'resize-handle-sidebar',
    () => document.getElementById('sidebar'),
    () => document.getElementById('workspace'),
    180
  );

  // Chat ↔ Editor resize
  setupResize(
    'resize-handle-editor',
    () => document.getElementById('chat-container'),
    () => document.getElementById('editor-container'),
    300
  );

  // Handle window resize to re-fit components
  window.addEventListener('resize', () => {
    if (terminalSpawned && !terminalPanel?.classList.contains('panel-hidden')) {
      fitAddon?.fit();
    }
  });
}

startApp();
