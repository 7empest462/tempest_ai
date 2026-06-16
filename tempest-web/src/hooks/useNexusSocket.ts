import { useEffect, useRef } from 'react';
import { useStore } from '../store';

export function useNexusSocket() {
  const socketRef = useRef<WebSocket | null>(null);
  const store = useStore();

  useEffect(() => {
    let reconnectTimer: any;
    
    const connect = () => {
      const socket = new WebSocket('ws://localhost:8080/ws');
      socketRef.current = socket;

      socket.onopen = () => {
        console.log("📡 [NEXUS]: Connection established.");
        store.setConnected(true);
        // Dispatch event to fetch explorer, history, and memories
        sendNexus('ListFiles', { path: '.' });
        sendNexus('GetHistory', {});
        sendNexus('GetMemories', {});
      };

      socket.onclose = () => {
        console.log("❌ [NEXUS]: Connection lost. Retrying...");
        store.setConnected(false);
        store.setStreaming(false);
        reconnectTimer = setTimeout(connect, 2000);
      };

      socket.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        handleMessage(msg);
      };
    };

    const handleMessage = (msg: any) => {
      switch (msg.type) {
        case 'History':
          store.setMessages(msg.payload.messages || []);
          break;
        case 'StreamToken':
          store.appendStreamContent(msg.payload.token);
          break;
        case 'ReasoningToken':
          store.appendReasoningContent(msg.payload.token);
          break;
        case 'Done':
          store.commitStream();
          break;
        case 'Telemetry':
          store.setMetrics(msg.payload.cpu, msg.payload.gpu, `${msg.payload.ram}`);
          break;
        case 'InferenceMetrics':
          if (msg.payload.tps != null) {
            store.setTps(`${msg.payload.tps} t/s`);
          }
          if (msg.payload.ctx_used != null) {
            store.setCtxUsed(msg.payload.ctx_used);
          }
          if (msg.payload.ctx_total != null) {
            store.setCtxTotal(msg.payload.ctx_total);
          }
          break;
        case 'FileTree':
          store.setExplorer(msg.payload.current_path, msg.payload.items);
          break;
        case 'FileContent':
          const filePath = msg.payload.path || 'unknown';
          const fileExt = filePath.split('.').pop() || '';
          store.setActiveFile({
            name: filePath,
            content: msg.payload.content,
            ext: fileExt
          });
          break;
        case 'TerminalOutput':
          window.dispatchEvent(new CustomEvent('terminal-output', { detail: msg.payload.data }));
          break;
        case 'BackendInfo':
          store.setBackendInfo(
            msg.payload.backend,
            msg.payload.planner,
            msg.payload.executor,
            msg.payload.verifier
          );
          break;
        case 'AgentStateChange':
          store.setAgentPhase(msg.payload.state);
          if (msg.payload.state === 'Done') {
            store.setActiveTools([]);
            store.setStreaming(false);
          }
          break;
        case 'ActiveTools':
          store.setActiveTools(msg.payload.tools);
          break;
        case 'ToolStart':
          store.addActiveToolExecution(msg.payload.name, msg.payload.args);
          break;
        case 'ToolResult':
          store.addToolResult({
            name: msg.payload.name,
            args: msg.payload.args,
            output: msg.payload.output,
            success: msg.payload.success
          });
          store.updateActiveToolExecution(
            msg.payload.name,
            msg.payload.args,
            msg.payload.success ? 'success' : 'error',
            msg.payload.output
          );
          if (msg.payload.name === 'store_memory' && msg.payload.success) {
            sendNexus('GetMemories', {});
          }
          break;
        case 'SafeModeRequest':
          console.log("🔒 [NEXUS]: SafeModeRequest received", msg.payload);
          store.setSafeModeRequest({
            rationale: msg.payload.rationale,
            diff: msg.payload.diff
          });
          break;
        case 'AskUserRequest':
          console.log("❓ [NEXUS]: AskUserRequest received", msg.payload);
          store.setAskUserRequest({
            question: msg.payload.question
          });
          break;
        case 'Memories':
          store.setMemories(msg.payload.memories || []);
          break;
        case 'TurnReviewRequest':
          console.log("🔍 [NEXUS]: TurnReviewRequest received", msg.payload);
          store.setTurnReviewRequest({
            diff: msg.payload.diff,
            files: msg.payload.files || []
          });
          break;
        case 'SearchResults':
          store.setSearchResults(msg.payload.matches || []);
          store.setSearching(false);
          break;
        case 'Error':
          store.appendStreamContent(`\n\n**System Error:** ${msg.payload.message}\n`);
          store.commitStream();
          store.clearReasoning();
          store.setStreaming(false);
          break;
      }
    };

    connect();

    return () => {
      clearTimeout(reconnectTimer);
      if (socketRef.current) {
        socketRef.current.close();
      }
    };
  }, []); // Run once on mount

  const sendNexus = (type: string, payload: any) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      socketRef.current.send(JSON.stringify({ type, payload }));
    }
  };

  // Expose sending to window for global access (like ChatInterface)
  // @ts-ignore
  window.sendNexus = sendNexus;

  return { sendNexus };
}
