import './style.css'
import * as wasm from './pkg/tempest_wasm.js'

declare var hljs: any;

const extensionMap: Record<string, string> = {
  'rs': 'rust',
  'ts': 'typescript',
  'js': 'javascript',
  'sh': 'bash',
  'toml': 'toml',
  'md': 'markdown',
  'json': 'json',
  'html': 'xml',
  'css': 'css',
  'py': 'python',
  'yml': 'yaml',
  'yaml': 'yaml',
  'zig': 'zig',
  'nix': 'nix'
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
        const width = window.innerWidth;
        const height = window.innerHeight;
        canvas.width = width;
        canvas.height = height;
        dashboard.resize(width, height);
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

  // 3. State
  let currentPath = '.';
  let currentOpenFile: string | null = null;
  let socket: WebSocket | null = null;
  let isEditing = false;
  let currentFileExt = '';

  // 4. WebSocket Connection
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
      console.warn("⚠️ Highlighting error, falling back to plaintext:", e);
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
        // Silently handle generic "Done" for writes
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
      case 'Error':
        appendMessage('system', `❌ [ERROR]: ${msg.payload.message}`);
        break;
    }
  };

  connect();

  // 5. Chat Logic
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

  // 6. File Explorer Logic
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
    const parentPath = `${currentPath}/..`;
    fetchExplorer(parentPath);
  });

  closeEditor?.addEventListener('click', () => {
    if (editorContainer) editorContainer.classList.add('hidden');
    currentOpenFile = null;
  });

  editBtn?.addEventListener('click', () => {
    if (!codeDisplay) return;
    
    isEditing = !isEditing;
    if (isEditing) {
      // Switch to editing mode
      codeDisplay.contentEditable = 'true';
      editBtn.innerText = 'SAVE';
      codeDisplay.focus();
      codeDisplay.className = 'hljs plaintext';
      codeDisplay.innerText = codeDisplay.innerText;
    } else {
      // Save changes
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

  // 7. Backend Toggle
  const backendRadios = document.querySelectorAll('input[name="backend"]');
  backendRadios.forEach(radio => {
    radio.addEventListener('change', (e) => {
      const target = e.target as HTMLInputElement;
      if (engineStatus) {
        engineStatus.innerText = `Engine: ${target.value.toUpperCase()}`;
      }
      appendMessage('system', `📡 [BACKEND]: Switching to ${target.value.toUpperCase()}...`);
    });
  });
}

startApp();
