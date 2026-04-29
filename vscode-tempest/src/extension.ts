/**
 * Copyright (c) 2026 Robert Simens. All Rights Reserved.
 * Licensed under the Tempest AI Source-Available License.
 * See LICENSE in the project root for full license information.
 */

import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';

export function activate(context: vscode.ExtensionContext) {
    console.log('Tempest AI is now active!');

    const provider = new TempestChatViewProvider(context.extensionUri);

    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(TempestChatViewProvider.viewType, provider)
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('tempest.focus', () => {
            vscode.commands.executeCommand('tempest.chatView.focus');
        })
    );
}

class TempestChatViewProvider implements vscode.WebviewViewProvider {
    public static readonly viewType = 'tempest.chatView';
    private _view?: vscode.WebviewView;
    private _tempestProcess?: ChildProcess;

    constructor(private readonly _extensionUri: vscode.Uri) {}

    private _disposables: vscode.Disposable[] = [];

    private _getEditorContext() {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return null;

        const document = editor.document;
        const selection = editor.selection;
        const selectedText = document.getText(selection);

        // Build a focused code window: selected text OR ±20 lines around cursor
        let visibleCode = '';
        if (selectedText.trim().length > 0) {
            visibleCode = selectedText;
        } else {
            const cursorLine = selection.active.line;
            const startLine = Math.max(0, cursorLine - 20);
            const endLine = Math.min(document.lineCount - 1, cursorLine + 20);
            const range = new vscode.Range(startLine, 0, endLine, document.lineAt(endLine).text.length);
            visibleCode = document.getText(range);
        }

        return {
            file_path: document.uri.fsPath,
            file_name: document.uri.fsPath.split('/').pop() || '',
            language_id: document.languageId,
            visible_code: visibleCode,
            has_selection: selectedText.trim().length > 0,
            cursor_line: selection.active.line + 1,
            total_lines: document.lineCount
        };
    }

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken,
    ) {
        this._view = webviewView;

        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this._extensionUri]
        };

        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

        // Clear previous listeners if re-resolving
        this._disposables.forEach(d => d.dispose());
        this._disposables = [];

        // Start backend if needed
        if (!this._tempestProcess) {
            this._startTempest();
        }

        // Attach message listener
        this._disposables.push(
            webviewView.webview.onDidReceiveMessage(data => {
                // console.log(`[Host] Received: ${data.type} (ID: ${data.id})`);
                switch (data.type) {
                    case 'tempest-request': {
                        const params = { ...data.payload.params };
                        
                        // Inject editor context for chat requests
                        if (data.payload.method === 'tempest/chat') {
                            params.editor_context = this._getEditorContext();
                        }

                        const rpc = JSON.stringify({
                            jsonrpc: "2.0",
                            id: data.id,
                            method: data.payload.method,
                            params: params
                        }) + "\n";
                        
                        if (this._tempestProcess && this._tempestProcess.stdin && this._tempestProcess.stdin.writable) {
                            // console.log(`[Host] Writing to Rust: ${data.payload.method}`);
                            this._tempestProcess.stdin.write(rpc);
                        } else {
                            console.error('[Host] Rust process not ready or stdin closed');
                            this._view?.webview.postMessage({ type: 'tempestStatus', value: 'Error: Backend not reachable.' });
                        }
                        break;
                    }
                }
            })
        );
    }

    private _startTempest() {
        // Use the absolute path to the release binary we just built
        const binaryPath = '/Volumes/Corsair_Lab/Home/Projects/tempest_ai/target/release/tempest_ai'; 
        
        this._tempestProcess = spawn(binaryPath, ['--mcp-server', '--mlx'], {
            cwd: vscode.workspace.workspaceFolders?.[0].uri.fsPath
        });

        this._tempestProcess.stdout?.on('data', (data: Buffer) => {
            const message = data.toString();
            const lines = message.split('\n').filter((l: string) => l.trim().length > 0);
            for (const line of lines) {
                try {
                    const json = JSON.parse(line);
                    if (json.id && (json.result || json.error)) {
                        this._view?.webview.postMessage({
                            type: 'tempest-response',
                            id: json.id,
                            payload: json.result || json.error
                        });
                    }
                    
                    if (json.method === 'tempest/thought') {
                        this._view?.webview.postMessage({ type: 'tempestThought', value: json.params.text });
                    } else if (json.method === 'tempest/status') {
                        this._view?.webview.postMessage({ type: 'tempestStatus', value: json.params.text });
                    } else if (json.result && json.result.content && json.result.content[0]) {
                        this._view?.webview.postMessage({ type: 'tempestResponse', value: json.result.content[0].text });
                    }
                } catch (e) {
                    this._view?.webview.postMessage({ type: 'tempestStatus', value: line });
                }
            }
        });

        this._tempestProcess.stderr?.on('data', (data: Buffer) => {
            console.error(`Tempest Error: ${data}`);
        });
    }

    private _sendToTempest(message: string) {
        const editor = vscode.window.activeTextEditor;
        let context = {};
        
        if (editor) {
            const document = editor.document;
            const selection = editor.selection;
            context = {
                activeFile: document.fileName,
                cursorLine: selection.active.line + 1,
                cursorChar: selection.active.character,
                selectionText: document.getText(selection),
                visibleRange: {
                    start: editor.visibleRanges[0]?.start.line + 1,
                    end: editor.visibleRanges[0]?.end.line + 1
                }
            };
        }

        const rpc = JSON.stringify({
            jsonrpc: "2.0",
            id: Date.now(),
            method: "tools/call",
            params: {
                name: "chat",
                arguments: { 
                    message,
                    editorContext: context
                }
            }
        }) + "\n";
        
        this._tempestProcess?.stdin?.write(rpc);
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        return `<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <style>
                    :root {
                        --bg: #0d1117;
                        --fg: #c9d1d9;
                        --accent: #58a6ff;
                        --border: #30363d;
                        --bubble-user: #161b22;
                        --bubble-tempest: #21262d;
                    }
                    body { 
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
                        background-color: var(--bg); 
                        color: var(--fg); 
                        font-size: 13px; 
                        margin: 0;
                        padding: 0;
                    }
                    .chat-container { display: flex; flex-direction: column; height: 100vh; }
                    #output { 
                        flex: 1; 
                        overflow-y: auto; 
                        padding: 16px; 
                        display: flex; 
                        flex-direction: column; 
                        gap: 12px;
                    }
                    .message { 
                        padding: 10px 14px; 
                        border-radius: 12px; 
                        max-width: 90%;
                        line-height: 1.6;
                        word-wrap: break-word;
                    }
                    .user { 
                        align-self: flex-end;
                        background: var(--bubble-user); 
                        border: 1px solid var(--border);
                        color: var(--accent);
                    }
                    .tempest { 
                        align-self: flex-start;
                        background: var(--bubble-tempest); 
                        border: 1px solid var(--border);
                    }
                    .thought { 
                        color: #8b949e; 
                        font-style: italic; 
                        font-size: 12px; 
                        border-left: 2px solid #30363d; 
                        padding-left: 10px; 
                        margin: 8px 0; 
                        background: rgba(48, 54, 61, 0.2);
                        padding: 8px;
                        border-radius: 4px;
                    }
                    .status-log { 
                        font-size: 10px; 
                        color: #8b949e; 
                        opacity: 0.8;
                        border-bottom: 1px solid var(--border);
                        padding-bottom: 4px;
                        margin-bottom: 8px;
                    }
                    #input-container { 
                        padding: 16px; 
                        background: #161b22; 
                        border-top: 1px solid var(--border);
                        display: flex;
                        gap: 10px;
                    }
                    #input { 
                        flex: 1;
                        padding: 10px 12px; 
                        background: #0d1117; 
                        border: 1px solid var(--border); 
                        color: white; 
                        outline: none; 
                        border-radius: 6px;
                        transition: border-color 0.2s;
                    }
                    #input:focus { border-color: var(--accent); }
                    #send-btn {
                        background: var(--accent);
                        color: white;
                        border: none;
                        border-radius: 6px;
                        padding: 0 16px;
                        font-weight: bold;
                        cursor: pointer;
                        transition: opacity 0.2s;
                    }
                    #send-btn:hover { opacity: 0.9; }
                </style>
            </head>
            <body>
                <div class="chat-container">
                    <div id="output">
                        <div class="status-log">🌪️ Tempest AI Initialized (Semantic Protocol v1.2)</div>
                    </div>
                    <div id="input-container">
                        <input type="text" id="input" placeholder="Ask Tempest..." autofocus />
                        <button id="send-btn">Send</button>
                    </div>
                </div>
                <script>
                    (function() {
                        var output = document.getElementById('output');
                        function log(msg, isError) {
                            var div = document.createElement('div');
                            div.className = 'status-log';
                            if (isError) div.style.color = '#ff7b72';
                            div.innerText = (isError ? '❌ ' : '> ') + msg;
                            output.appendChild(div);
                            output.scrollTop = output.scrollHeight;
                        }

                        window.onerror = function(msg, url, line) {
                            log('JS Error: ' + msg + ' (Line: ' + line + ')', true);
                        };

                        try {
                            var vscode = acquireVsCodeApi();
                        } catch (e) {
                            log('Failed to acquire VS Code API: ' + e.message, true);
                        }

                        var input = document.getElementById('input');
                        var sendBtn = document.getElementById('send-btn');
                        
                        var currentResponseDiv = null;
                        var currentThoughtDiv = null;

                        function handleStream(chunk) {
                            if (chunk.reasoning) {
                                if (!currentThoughtDiv) {
                                    currentThoughtDiv = document.createElement('div');
                                    currentThoughtDiv.className = 'thought';
                                    currentThoughtDiv.innerHTML = '<div style="font-weight:bold;margin-bottom:4px;font-size:10px;opacity:0.6;">🧠 THOUGHT PROCESS</div>';
                                    output.appendChild(currentThoughtDiv);
                                }
                                var span = document.createElement('span');
                                span.innerText = chunk.reasoning;
                                currentThoughtDiv.appendChild(span);
                            }
                            if (chunk.content) {
                                if (!currentResponseDiv) {
                                    currentResponseDiv = document.createElement('div');
                                    currentResponseDiv.className = 'message tempest';
                                    currentResponseDiv.innerHTML = '<b>Tempest:</b><br/>';
                                    output.appendChild(currentResponseDiv);
                                }
                                currentResponseDiv.innerHTML += chunk.content.replace(/\\n/g, '<br/>');
                            }
                            output.scrollTop = output.scrollHeight;
                            if (chunk.done) {
                                currentResponseDiv = null;
                                currentThoughtDiv = null;
                            }
                        }

                        function sendMessage() {
                            try {
                                var val = input.value;
                                if (!val) return;
                                input.value = '';
                                
                                var userMsg = document.createElement('div');
                                userMsg.className = 'message user';
                                userMsg.innerHTML = '<b>You:</b><br/>' + val;
                                output.appendChild(userMsg);
                                output.scrollTop = output.scrollHeight;

                                currentResponseDiv = null;
                                currentThoughtDiv = null;

                                vscode.postMessage({ 
                                    type: 'tempest-request', 
                                    id: Date.now(), 
                                    payload: { method: 'tempest/chat', params: { message: val } } 
                                });
                                // log('Message sent to host.'); // Removed to keep UI clean
                            } catch (e) {
                                log('sendMessage error: ' + e.message, true);
                            }
                        }

                        if (sendBtn) {
                            sendBtn.addEventListener('click', sendMessage);
                        }
                        if (input) {
                            input.addEventListener('keydown', function(e) {
                                if (e.key === 'Enter') sendMessage();
                            });
                        }

                        window.addEventListener('message', function(event) {
                            var data = event.data;
                            if (data.type === 'tempest-response') {
                                if (data.payload && data.payload.method === 'tempest/chat') {
                                    handleStream(data.payload.payload);
                                }
                            } else if (data.type === 'tempestStatus') {
                                log(data.value);
                            } else if (data.type === 'tempestThought') {
                                if (!currentThoughtDiv) {
                                    currentThoughtDiv = document.createElement('div');
                                    currentThoughtDiv.className = 'thought';
                                    currentThoughtDiv.innerHTML = '<div style="font-weight:bold;margin-bottom:4px;font-size:10px;opacity:0.6;">🧠 THOUGHT PROCESS</div>';
                                    output.appendChild(currentThoughtDiv);
                                }
                                var span = document.createElement('span');
                                span.innerText = data.value;
                                currentThoughtDiv.appendChild(span);
                                output.scrollTop = output.scrollHeight;
                            }
                        });
                    })();
                </script>
            </body>
            </html>`;
    }
}
