/**
 * Copyright (c) 2026 Robert Simens. All Rights Reserved.
 * Licensed under the Tempest AI Source-Available License.
 * See LICENSE in the project root for full license information.
 */

import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as readline from 'readline';
import * as fs from 'fs';
import * as os from 'os';

let activeProvider: TempestChatViewProvider | undefined;
let statusBarItem: vscode.StatusBarItem;

export function activate(context: vscode.ExtensionContext) {
    console.log('Tempest AI is now active!');

    activeProvider = new TempestChatViewProvider(context.extensionUri);
    
    // Initialize Status Bar
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBarItem.command = 'tempest.focus';
    statusBarItem.text = '$(tornado) Tempest: Ready';
    statusBarItem.tooltip = 'Click to open Tempest Chat';
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(TempestChatViewProvider.viewType, activeProvider)
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('tempest.focus', () => {
            vscode.commands.executeCommand('tempest.chatView.focus');
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('tempest.clearHistory', async () => {
            const historyPath = path.join(os.homedir(), 'Library', 'Application Support', 'tempest_ai', 'history.json');
            
            // 1. Wipe in-memory logs and kill process
            if (activeProvider) {
                activeProvider.refresh(); 
            }

            // 2. Delete the physical file
            if (fs.existsSync(historyPath)) {
                try {
                    fs.unlinkSync(historyPath);
                    vscode.window.showInformationMessage('Tempest AI: History wiped and backend reset.');
                } catch (e) {
                    vscode.window.showErrorMessage(`Failed to delete history file: ${e}`);
                }
            } else {
                vscode.window.showInformationMessage('Tempest AI: Backend reset (no history file found).');
            }
        })
    );
}

export function deactivate() {
    if (activeProvider) {
        activeProvider.dispose();
    }
}

class TempestChatViewProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = 'tempest.chatView';
    private _view?: vscode.WebviewView;
    private _tempestProcess?: ChildProcess;

    constructor(private readonly _extensionUri: vscode.Uri) {}

    private _disposables: vscode.Disposable[] = [];
    
    public dispose() {
        if (this._tempestProcess) {
            console.log('[Host] Killing Tempest process...');
            this._tempestProcess.kill();
            this._tempestProcess = undefined;
        }
        this._disposables.forEach(d => d.dispose());
        this._disposables = [];
        this._logHistory = [];
        this._isWebviewReady = false;
    }

    public refresh() {
        this._logHistory = [];
        if (this._tempestProcess) {
            this._tempestProcess.kill();
            this._tempestProcess = undefined;
        }
        if (this._view) {
            this._view.webview.html = this._getHtmlForWebview(this._view.webview);
        }
    }

    private _getEditorContext(): any {
        const editor = vscode.window.activeTextEditor;
        
        if (!editor) {
            return {
                file_path: "unknown",
                language: "unknown",
                content: "",
                cursor_line: 0
            };
        }

        const document = editor.document;
        const selection = editor.selection;

        return {
            file_name: path.basename(document.fileName),
            file_path: document.fileName,
            language_id: document.languageId,
            content: document.getText(),
            selected_text: document.getText(selection),
            cursor_line: selection.active.line + 1,
            cursor_column: selection.active.character + 1,
            visible_code: document.getText() // Map to visible_code for backend compatibility
        };
    }

    private _pendingRequests = new Map<string | number, string>();
    private _isWebviewReady = false;
    private _logHistory: string[] = []; // PERSISTENT history

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken,
    ) {
        this._view = webviewView;
        this._isWebviewReady = false; // Reset for new view

        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this._extensionUri]
        };

        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);
        
        // Start backend if not already running
        if (!this._tempestProcess) {
            this._startTempest();
        }

        // Attach message listener
        this._disposables.push(
            webviewView.webview.onDidReceiveMessage(async (data) => {
                if (data.type === 'webview-ready') {
                    console.log('[Host] Webview signaled READY');
                    this._isWebviewReady = true;
                    if (this._logHistory.length > 0) {
                        this._logHistory.forEach(log => {
                            webviewView.webview.postMessage({ type: 'tempestThought', value: log });
                        });
                    }
                    return;
                }

                if (data.type === 'tempest-request' && data.id) {
                    let params = data.payload?.params || {};
                    const method = data.payload?.method || 'tempest/chat';

                    // === AUTO CONTEXT LOGIC ===
                    if (params.auto_context === true || method === 'tempest/chat') {
                        params.editor_context = this._getEditorContext();
                        console.log('[Host] Injected editor context');
                    }

                    const rpcObj = {
                        jsonrpc: "2.0",
                        id: data.id,
                        method: method,
                        params: params
                    };

                    this._pendingRequests.set(data.id, method);

                    if (this._tempestProcess?.stdin?.writable) {
                        this._tempestProcess.stdin.write(JSON.stringify(rpcObj) + "\n");
                    } else {
                        console.error('[Host] Tempest process stdin not writable');
                    }
                }
            })
        );
    }

    private _startTempest() {
        const config = vscode.workspace.getConfiguration('tempest');
        const binaryPath = config.get<string>('binaryPath') || '/Volumes/Corsair_Lab/Home/Projects/tempest_ai/target/release/tempest_ai';
        const useMlx = config.get<boolean>('useMlx') !== false; // Default to true if not set
        const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || process.env.HOME || '/tmp';

        console.log(`[Host] Spawning Tempest: ${binaryPath} (MLX: ${useMlx}) in ${cwd}`);
        
        const args = ['--mcp-server'];
        if (useMlx) args.push('--mlx');

        this._tempestProcess = spawn(binaryPath, args, { cwd });

        this._tempestProcess.on('error', (err) => {
            console.error('[Host] Failed to start Tempest process:', err);
            if (this._view) {
                this._view.webview.postMessage({ type: 'tempestThought', value: `\n❌ ERROR: Failed to start backend: ${err.message}\n` });
            }
        });

        this._tempestProcess.on('exit', (code, signal) => {
            console.log(`[Host] Tempest process exited with code ${code} and signal ${signal}`);
            if (this._view) {
                this._view.webview.postMessage({ type: 'tempestThought', value: `\n⚠️ Backend exited (Code: ${code}, Signal: ${signal})\n` });
            }
        });

        // Combined listener for Zero-Latency and JSON-RPC
        const rl = readline.createInterface({
            input: this._tempestProcess.stdout!,
            terminal: false
        });

        rl.on('line', (line: string) => {
            const trimmed = line.trim();
            if (!trimmed) return;

            // DIAGNOSTIC: Log every single line received to the Debug Console
            console.log(`[Backend -> Host] ${trimmed}`);

            // Log everything to history regardless of type
            this._logHistory.push(line + '\n');

            if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
                try {
                    const json = JSON.parse(trimmed);
                    const method = json.method || (json.id ? this._pendingRequests.get(json.id) : undefined);
                    if (json.id) {
                        // For streaming, we keep the ID until the backend sends a final result.
                        // But since we switched to notifications for tokens, we can clear it after the ACK.
                        this._pendingRequests.delete(json.id);
                    }

                    if (this._view && this._isWebviewReady) {
                        this._view.webview.postMessage({
                            type: 'tempest-response',
                            id: json.id,
                            method: method,
                            payload: json
                        });

                        // Special Case: Bridge notifications to the phase-tracking system
                        if (method === 'tempest/status') {
                            const statusText = json.params?.text || '';
                            let phase = 'READY';
                            
                            if (statusText.toLowerCase().includes('thinking')) phase = 'THINKING';
                            if (statusText.toLowerCase().includes('analyzing')) phase = 'ANALYZING';
                            if (statusText.toLowerCase().includes('grounding')) phase = 'GROUNDING';
                            if (statusText.toLowerCase().includes('metal') || statusText.toLowerCase().includes('warm')) phase = 'LOADING';
                            if (statusText.toLowerCase().includes('ready')) phase = 'READY';

                            this._view.webview.postMessage({
                                type: 'tempest-response',
                                payload: { phase: phase }
                            });
                        }
                    }
                    
                    if (method === 'tempest/status') {
                        statusBarItem.text = `$(tornado) Tempest: ${json.params?.text || 'Ready'}`;
                    } else if (method === 'tempest/thought' || (json.result?.ChatResponse?.payload?.reasoning)) {
                        statusBarItem.text = `$(sync~spin) Tempest: Thinking...`;
                    } else if (method === 'tempest/edit') {
                        const params = json.params || json.result?.payload || {};
                        if (params.path && params.content) {
                            this._applyEditorEdit(params.path, params.content);
                        }
                    }
                } catch (e) {
                    // Not valid JSON, treat as raw log below
                }
            } else {
                // Raw log
                if (this._view && this._isWebviewReady) {
                    this._view.webview.postMessage({
                        type: 'tempestThought',
                        value: line + '\n'
                    });
                }
            }
        });

        this._tempestProcess.stderr?.on('data', (data: Buffer) => {
            console.error(`Tempest Error: ${data}`);
        });
    }

    private async _applyEditorEdit(filePath: string, content: string) {
        const uri = vscode.Uri.file(filePath);
        const edit = new vscode.WorkspaceEdit();
        
        // Replace entire content for now
        const fullRange = new vscode.Range(
            new vscode.Position(0, 0),
            new vscode.Position(100000, 0)
        );
        
        edit.replace(uri, fullRange, content);
        await vscode.workspace.applyEdit(edit);
    }

    private _getHtmlForWebview(webview: vscode.Webview): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Tempest AI</title>
    <script src="https://cdn.jsdelivr.net/npm/vue@3.4.0/dist/vue.global.prod.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
    <style>
        :root {
            --tempest-bg: var(--vscode-sideBar-background);
            --tempest-fg: var(--vscode-foreground);
            --tempest-accent: #4da6ff;
            --tempest-success: #00cc88;
            --tempest-border: var(--vscode-sideBar-border);
            --tempest-glass: var(--vscode-editor-background);
            --tempest-text: var(--vscode-editor-foreground);
            --vortex-glow: #0088ff;
        }

        body {
            margin: 0; padding: 0;
            background: var(--tempest-bg);
            color: var(--tempest-fg);
            font-family: var(--vscode-font-family);
            height: 100vh;
            overflow: hidden;
        }

        #app { height: 100%; display: flex; flex-direction: column; }

        .tempest-app { height: 100%; display: flex; flex-direction: column; }
        .header { padding: 12px 16px; border-bottom: 1px solid var(--tempest-border); display: flex; align-items: center; justify-content: space-between; flex-shrink: 0; }
        .logo { display: flex; align-items: center; gap: 8px; font-size: 1.2rem; font-weight: 600; }
        .tornado { font-size: 24px; }
        .title { background: linear-gradient(90deg, #4da6ff, #00ffaa); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }
        .status { font-size: 0.7rem; padding: 2px 8px; border-radius: 12px; font-weight: 500; background: rgba(77, 166, 255, 0.1); color: var(--tempest-accent); }
        
        .tabs { display: flex; border-bottom: 1px solid var(--tempest-border); }
        .tabs button { flex: 1; padding: 10px 0; background: transparent; border: none; color: inherit; cursor: pointer; font-size: 0.85rem; opacity: 0.7; }
        .tabs button.active { border-bottom: 2px solid var(--tempest-accent); color: var(--tempest-accent); opacity: 1; }

        .chat-tab, .upload-tab { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
        .chat-output { flex: 1; padding: 12px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }
        
        .message { padding: 8px 12px; border-radius: 6px; max-width: 90%; font-size: 13px; line-height: 1.5; word-break: break-word; }
        .message.user { align-self: flex-end; background: var(--tempest-accent); color: white; }
        .message.tempest { align-self: flex-start; background: var(--tempest-glass); border: 1px solid var(--tempest-border); color: var(--tempest-text); }
        .message pre { background: rgba(0,0,0,0.2); padding: 8px; border-radius: 4px; overflow-x: auto; }
        .message code { font-family: var(--vscode-editor-font-family); }

        .thought-block { margin: 4px 0; padding: 10px; background: rgba(255,255,255,0.04); border-left: 2px solid var(--tempest-accent); font-size: 12px; color: var(--vscode-descriptionForeground); border-radius: 0 4px 4px 0; }
        .thought-header { font-weight: bold; font-size: 10px; margin-bottom: 6px; letter-spacing: 0.5px; opacity: 0.8; }
        .thought-content { white-space: pre-wrap; line-height: 1.4; }

        .input-area { padding: 12px; display: flex; gap: 8px; background: var(--tempest-bg); border-top: 1px solid var(--tempest-border); }
        .chat-input { flex: 1; padding: 8px 12px; background: var(--vscode-input-background); color: var(--vscode-input-foreground); border: 1px solid var(--vscode-input-border); border-radius: 4px; font-size: 13px; }
        .send-btn { width: 32px; height: 32px; background: var(--tempest-accent); color: white; border: none; border-radius: 4px; cursor: pointer; display: flex; align-items: center; justify-content: center; font-size: 18px; }

        .spinner { display: inline-block; animation: spin 2s linear infinite; }
        @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }

        /* Pulsing "Thinking" Effect */
        .thought-block.pulsing { animation: breathe 2s ease-in-out infinite; opacity: 0.8; }
        @keyframes breathe { 0%, 100% { opacity: 0.4; transform: scale(0.98); } 50% { opacity: 0.9; transform: scale(1.0); } }

        .message.streaming { border-right: 2px solid var(--tempest-accent); animation: blink 0.8s step-end infinite; }
        @keyframes blink { from, to { border-color: transparent; } 50% { border-color: var(--tempest-accent); } }

        /* Smart Toolbar */
        .toolbar { display: flex; gap: 6px; padding: 10px; background: rgba(0,0,0,0.2); border-bottom: 1px solid rgba(255,255,255,0.05); flex-wrap: wrap; }
        .tool-btn { flex: 1; padding: 6px 4px; background: rgba(77, 166, 255, 0.1); color: #ccc; border: 1px solid rgba(77, 166, 255, 0.2); border-radius: 4px; font-size: 11px; cursor: pointer; transition: all 0.2s; white-space: nowrap; }
        .tool-btn:hover:not(:disabled) { background: var(--tempest-accent); color: white; border-color: var(--tempest-accent); }
        .tool-btn:disabled { opacity: 0.5; cursor: not-allowed; }

        /* Upload Styles */
        .drop-area { flex: 1; margin: 20px; border: 2px dashed #444; border-radius: 12px; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; background: rgba(255,255,255,0.02); cursor: pointer; }
        .drop-area.drag-over { border-color: var(--tempest-accent); background: rgba(77, 166, 255, 0.05); }
        .drop-icon { font-size: 40px; margin-bottom: 12px; }

        /* Vortex Styles */
        .vortex-tab { flex: 1; display: flex; flex-direction: column; overflow: hidden; position: relative; }
        #vortex-canvas { width: 100%; height: 100%; min-height: 300px; display: block; border-radius: 8px; box-shadow: 0 0 20px rgba(0, 136, 255, 0.1); }
        .vortex-overlay { position: absolute; top: 16px; left: 16px; pointer-events: none; }
        .vortex-label { font-size: 10px; font-weight: bold; color: var(--vortex-glow); text-transform: uppercase; letter-spacing: 2px; }
    </style>
</head>
<body>
    <div id="app"></div>

    <script>
        const { createApp, ref, onMounted, nextTick } = Vue;

        const App = {
            template: \`
                <div class="tempest-app">
                    <div class="header">
                        <div class="logo">
                            <span class="tornado">🌪️</span>
                            <span class="title">TEMPEST</span>
                        </div>
                        <div class="status">
                            <span v-if="isLoading" class="spinner">🌀</span>
                            {{ activeModel }} • {{ currentPhase }}
                        </div>
                    </div>

                    <!-- Smart Toolbar -->
                    <div class="toolbar">
                        <button @click="quickAction('Fix all issues in this file and make it cleaner')" class="tool-btn" :disabled="isLoading">🔧 Fix</button>
                        <button @click="quickAction('Explain what this code does in detail')" class="tool-btn" :disabled="isLoading">📖 Explain</button>
                        <button @click="quickAction('Refactor this code to be more readable and efficient')" class="tool-btn" :disabled="isLoading">⚡ Refactor</button>
                        <button @click="quickAction('Add clear comments and documentation to this code')" class="tool-btn" :disabled="isLoading">💬 Comment</button>
                    </div>

                    <div class="tabs">
                        <button :class="{ active: activeTab === 'chat' }" @click="activeTab = 'chat'">💬 CHAT</button>
                        <button :class="{ active: activeTab === 'upload' }" @click="activeTab = 'upload'">📤 UPLOAD</button>
                        <button :class="{ active: activeTab === 'vortex' }" @click="activeTab = 'vortex'">🌀 VORTEX</button>
                    </div>

                    <div v-show="activeTab === 'chat'" class="chat-tab">
                        <div ref="outputRef" class="chat-output">
                            <div class="message tempest">⚡ Tempest Bridge Active. System Ready.</div>
                            
                            <template v-for="msg in messages" :key="msg.id">
                                <div v-if="msg.thoughts" class="thought-block">
                                    <div class="thought-header">THOUGHTS</div>
                                    <div class="thought-content">{{ msg.thoughts }}</div>
                                </div>
                                <div class="message" :class="msg.type" v-html="renderMarkdown(msg.text)"></div>
                            </template>
                            
                            <!-- Current Streaming Message -->
                            <div v-if="streamingThoughts" class="thought-block">
                                <div class="thought-header">THOUGHTS</div>
                                <div class="thought-content">{{ streamingThoughts }}</div>
                            </div>
                            <div v-if="streamingText" class="message tempest streaming" v-html="renderMarkdown(streamingText)"></div>
                            
                            <!-- Bottom Loading Indicator -->
                            <div v-if="isLoading && !streamingText" class="thought-block pulsing">
                                <div class="thought-header">SYSTEM</div>
                                <div class="thought-content">Agent is thinking...</div>
                            </div>
                        </div>

                        <div class="input-area">
                            <input 
                                v-model="inputMsg" 
                                @keydown.enter="send" 
                                placeholder="Storm your code..." 
                                class="chat-input"
                                :disabled="isLoading"
                            />
                            <button @click="send" class="send-btn" :disabled="isLoading">
                                <span v-if="isLoading">🌀</span>
                                <span v-else>↑</span>
                            </button>
                        </div>
                    </div>

                    <div v-if="activeTab === 'upload'" class="upload-tab">
                        <div 
                            class="drop-area" 
                            :class="{ 'drag-over': isDragging }"
                            @dragover.prevent="isDragging = true"
                            @dragleave.prevent="isDragging = false"
                            @drop.prevent="onDrop"
                        >
                            <div class="drop-icon">📤</div>
                            <h3>Drop files here</h3>
                            <p>or click to select</p>
                        </div>
                    </div>

                    <div v-show="activeTab === 'vortex'" class="vortex-tab">
                        <div class="vortex-overlay">
                            <div class="vortex-label">Tempest Vortex Engine</div>
                            <div style="font-size: 8px; opacity: 0.6;">Local GPU / WebGPU Accelerated</div>
                        </div>
                        <canvas id="vortex-canvas"></canvas>
                        <div v-if="vortexError" style="padding: 20px; text-align: center; color: #ff6666;">
                            ⚠️ {{ vortexError }}
                        </div>
                    </div>
                </div>
            \`,
            setup() {
                const vscode = acquireVsCodeApi();
                const activeTab = ref('chat');
                const inputMsg = ref('');
                const messages = ref([]);
                const streamingText = ref('');
                const streamingThoughts = ref('');
                const isLoading = ref(false);
                const currentPhase = ref('IDLE');
                const activeModel = ref('TEMPEST');
                const outputRef = ref(null);
                const isDragging = ref(false);
                const vortexError = ref(null);
                const isVortexInitialized = ref(false);

                const renderMarkdown = (text) => marked.parse(text);

                const scrollToBottom = async (force = false) => {
                    await nextTick();
                    if (outputRef.value) {
                        const { scrollTop, scrollHeight, clientHeight } = outputRef.value;
                        const isAtBottom = scrollHeight - scrollTop - clientHeight < 100;
                        if (force || isAtBottom) {
                            outputRef.value.scrollTop = outputRef.value.scrollHeight;
                        }
                    }
                };

                const send = () => {
                    const text = inputMsg.value.trim();
                    if (!text || isLoading.value) return;
                    
                    console.log('[Webview] Sending message:', text);
                    messages.value.push({ id: Date.now(), type: 'user', text });
                    inputMsg.value = '';
                    isLoading.value = true;
                    currentPhase.value = 'LOADING';
                    scrollToBottom(true);

                    vscode.postMessage({
                        type: 'tempest-request',
                        id: Date.now(),
                        payload: {
                            method: 'tempest/chat',
                            params: { message: text }
                        }
                    });
                };

                const onDrop = (e) => {
                    isDragging.value = false;
                    const files = Array.from(e.dataTransfer.files);
                    files.forEach(f => {
                        vscode.postMessage({
                            type: 'tempest-request',
                            id: Date.now(),
                            payload: { method: 'tempest/file_uploaded', params: { fileName: f.name } }
                        });
                    });
                };

                const quickAction = (action) => {
                    if (isLoading.value) return;
                    messages.value.push({ id: Date.now(), type: 'user', text: action });
                    isLoading.value = true;
                    currentPhase.value = 'LOADING';
                    scrollToBottom(true);
                    vscode.postMessage({
                        type: 'tempest-request',
                        id: Date.now(),
                        payload: {
                            method: 'tempest/chat',
                            params: { message: action, auto_context: true }
                        }
                    });
                };

                onMounted(() => {
                    window.addEventListener('message', event => {
                        const data = event.data;
                        if (data.type === 'tempest-response') {
                            const findV = (obj, key) => {
                                if (!obj || typeof obj !== 'object') return null;
                                if (obj[key] !== undefined) return obj[key];
                                return findV(obj.payload, key) || findV(obj.result, key);
                            };

                            const raw = data.payload?.result || data.payload?.params || data.payload;
                            const reasoning = findV(raw, 'reasoning');
                            const content = findV(raw, 'content') || findV(raw, 'text') || findV(raw, 'value');
                            const phase = findV(raw, 'phase');
                            const modelUpdate = findV(raw, 'model');
                            const done = findV(raw, 'done');

                            if (phase) currentPhase.value = phase;
                            if (modelUpdate) activeModel.value = modelUpdate;
                            
                            // Heuristic: Extract model name from status text if present
                            const statusText = findV(raw, 'text') || '';
                            if (statusText.includes('[Using ') && statusText.includes(']')) {
                                const match = statusText.match(/\[Using (.*?)\]/);
                                if (match && match[1]) {
                                    activeModel.value = match[1].split(':')[0].toUpperCase();
                                }
                            }

                            if (reasoning) streamingThoughts.value += reasoning;
                            if (content) streamingText.value += content;
                            
                            if (done) {
                                messages.value.push({
                                    id: Date.now(),
                                    type: 'tempest',
                                    text: streamingText.value,
                                    thoughts: streamingThoughts.value
                                });
                                streamingText.value = '';
                                streamingThoughts.value = '';
                                isLoading.value = false;
                                currentPhase.value = 'IDLE';
                            }
                            scrollToBottom();
                        }
                    });
                    vscode.postMessage({ type: 'webview-ready' });
                });

                Vue.watch(activeTab, async (newTab) => {
                    if (newTab === 'vortex' && !isVortexInitialized.value) {
                        try {
                            const wasmUri = "${webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'resources', 'wasm', 'tempest_wasm.js'))}";
                            const { default: init, initialize_dashboard } = await import(wasmUri);
                            await init();
                            await initialize_dashboard('vortex-canvas');
                            isVortexInitialized.value = true;
                        } catch (e) {
                            console.error('[Webview] Vortex initialization failed:', e);
                            vortexError.value = "WebGPU Initialization Failed. Ensure your browser/system supports WebGPU.";
                        }
                    }
                });

                return { 
                    activeTab, inputMsg, messages, streamingText, streamingThoughts, 
                    isLoading, currentPhase, activeModel, outputRef, isDragging,
                    vortexError,
                    send, renderMarkdown, onDrop, quickAction
                };
            }
        };

        createApp(App).mount('#app');
    </script>
</body>
</html>`;
    }
}
