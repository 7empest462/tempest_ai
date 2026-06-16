import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type AgentPhase = 'Idle' | 'Thinking' | 'Planning' | 'Executing' | 'PendingTools' | 'ExecutingTools' | 'StreamingContent' | 'Done';

export interface ToolCallResult {
  name: string;
  args?: string;
  output?: string;
  success: boolean;
}

export interface ChatMessage {
  id: string;
  role: 'system' | 'ai' | 'user';
  content: string;
  reasoning?: string;
  tools?: ToolCallResult[];
}

export interface WebMemoryItem {
  topic: string;
  content: string;
  tags?: string;
  updated_at: string;
}

export interface ActiveToolExecution {
  id: string;
  name: string;
  args?: string;
  status: 'running' | 'success' | 'error';
  output?: string;
  progress: number;
}

export interface WebFileDiff {
  path: string;
  original: string;
  modified: string;
}

export interface FileItem {
  name: string;
  is_dir: boolean;
}

export type BackgroundIntensity = 'subtle' | 'medium' | 'full';

interface TempestState {
  // Connection
  isConnected: boolean;
  engineStatus: string;
  plannerModel: string;
  executorModel: string;
  verifierModel: string;
  setConnected: (val: boolean) => void;
  setEngineStatus: (status: string) => void;
  setBackendInfo: (backend: string, planner: string, executor: string, verifier: string) => void;

  // Metrics
  cpu: number;
  gpu: number;
  ram: string;
  tps: string;
  ctxUsed: number;
  ctxTotal: number;
  setMetrics: (cpu: number, gpu: number, ram: string) => void;
  setTps: (tps: string) => void;
  setCtxUsed: (ctx: number) => void;
  setCtxTotal: (ctx: number) => void;

  // Agent Lifecycle
  agentPhase: AgentPhase;
  currentTask: string;
  activeTools: string[];
  setAgentPhase: (phase: AgentPhase) => void;
  setCurrentTask: (task: string) => void;
  setActiveTools: (tools: string[]) => void;

  // Chat
  messages: ChatMessage[];
  isStreaming: boolean;
  streamAccumulator: string;
  safeModeRequest: { rationale: string; diff: string } | null;
  askUserRequest: { question: string } | null;
  addMessage: (msg: ChatMessage) => void;
  setMessages: (messages: ChatMessage[]) => void;
  updateLastMessage: (content: string) => void;
  setStreaming: (val: boolean) => void;
  appendStreamContent: (chunk: string) => void;
  commitStream: () => void;
  setSafeModeRequest: (req: { rationale: string; diff: string } | null) => void;
  setAskUserRequest: (req: { question: string } | null) => void;

  // Memories
  memories: WebMemoryItem[];
  setMemories: (memories: WebMemoryItem[]) => void;

  // Active Tool Executions
  activeToolExecutions: ActiveToolExecution[];
  addActiveToolExecution: (name: string, args?: string) => void;
  updateActiveToolExecution: (name: string, args: string | undefined, status: 'success' | 'error', output?: string) => void;
  clearActiveToolExecutions: () => void;

  // Turn Review Request
  turnReviewRequest: { diff: string; files: WebFileDiff[] } | null;
  setTurnReviewRequest: (req: { diff: string; files: WebFileDiff[] } | null) => void;

  // File Explorer
  currentPath: string;
  fileItems: FileItem[];
  setExplorer: (path: string, items: FileItem[]) => void;

  // Editor Focus
  activeFile: { name: string, content: string, ext: string } | null;
  setActiveFile: (file: { name: string, content: string, ext: string } | null) => void;
  updateActiveFileContent: (content: string) => void;
  isFileEditable: boolean;
  setFileEditable: (val: boolean) => void;
  
  // Layout States
  isEditorFocused: boolean;
  setEditorFocused: (val: boolean) => void;
  isTerminalOpen: boolean;
  setTerminalOpen: (val: boolean) => void;

  // Settings
  backgroundIntensity: BackgroundIntensity;
  setBackgroundIntensity: (intensity: BackgroundIntensity) => void;

  // Active Tab
  activeTab: 'files' | 'agent' | 'search' | 'settings';
  setActiveTab: (tab: 'files' | 'agent' | 'search' | 'settings') => void;

  // View Mode
  chatViewMode: 'classic' | 'timeline';
  setChatViewMode: (mode: 'classic' | 'timeline') => void;

  // Reasoning & Tools
  reasoningAccumulator: string;
  appendReasoningContent: (chunk: string) => void;
  clearReasoning: () => void;
  currentToolResults: ToolCallResult[];
  addToolResult: (res: ToolCallResult) => void;
  clearToolResults: () => void;

  // Code Search
  searchResults: any[];
  isSearching: boolean;
  setSearchResults: (results: any[]) => void;
  setSearching: (val: boolean) => void;
}

export const useStore = create<TempestState>()(
  persist(
    (set) => ({
  isConnected: false,
  engineStatus: 'Initializing...',
  plannerModel: '--',
  executorModel: '--',
  verifierModel: '--',
  setConnected: (val) => set({ isConnected: val }),
  setEngineStatus: (status) => set({ engineStatus: status }),
  setBackendInfo: (backend, planner, executor, verifier) => set({
    engineStatus: backend,
    plannerModel: planner,
    executorModel: executor,
    verifierModel: verifier
  }),

  cpu: 0,
  gpu: 0,
  ram: '--',
  tps: 'idle',
  ctxUsed: 0,
  ctxTotal: 32768,
  setMetrics: (cpu, gpu, ram) => set({ cpu, gpu, ram }),
  setTps: (tps) => set({ tps }),
  setCtxUsed: (ctx) => set({ ctxUsed: ctx }),
  setCtxTotal: (ctx) => set({ ctxTotal: ctx }),

  agentPhase: 'Idle',
  currentTask: '--',
  activeTools: [],
  setAgentPhase: (phase) => set({ agentPhase: phase }),
  setCurrentTask: (task) => set({ currentTask: task }),
  setActiveTools: (tools) => set({ activeTools: tools }),

  messages: [{ id: 'init', role: 'system', content: '🌪️ [SYSTEM]: Neural link established. Environment grounded.' }],
  isStreaming: false,
  streamAccumulator: '',
  safeModeRequest: null,
  askUserRequest: null,
  addMessage: (msg) => set((state) => ({ messages: [...state.messages, msg] })),
  setMessages: (messages) => set({ messages }),
  updateLastMessage: (content) => set((state) => {
    const newMsgs = [...state.messages];
    if (newMsgs.length > 0) {
      newMsgs[newMsgs.length - 1].content = content;
    }
    return { messages: newMsgs };
  }),
  setStreaming: (val) => set({ isStreaming: val }),
  appendStreamContent: (chunk) => set((state) => ({ streamAccumulator: state.streamAccumulator + chunk })),
  commitStream: () => set((state) => {
    if (!state.streamAccumulator && !state.reasoningAccumulator && state.currentToolResults.length === 0) {
      return { isStreaming: false };
    }
    return {
      messages: [...state.messages, { 
        id: Date.now().toString(), 
        role: 'ai', 
        content: state.streamAccumulator,
        reasoning: state.reasoningAccumulator,
        tools: state.currentToolResults 
      }],
      streamAccumulator: '',
      reasoningAccumulator: '',
      currentToolResults: [],
      isStreaming: false
    };
  }),
  setSafeModeRequest: (req) => set({ safeModeRequest: req }),
  setAskUserRequest: (req) => set({ askUserRequest: req }),

  // Memories
  memories: [],
  setMemories: (memories) => set({ memories }),

  // Active Tool Executions
  activeToolExecutions: [],
  addActiveToolExecution: (name, args) => set((state) => {
    const id = `${name}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    const newExec: ActiveToolExecution = {
      id,
      name,
      args,
      status: 'running',
      progress: 15,
    };
    return { activeToolExecutions: [...state.activeToolExecutions, newExec] };
  }),
  updateActiveToolExecution: (name, args, status, output) => set((state) => {
    const executions = [...state.activeToolExecutions];
    let idx = -1;
    if (args) {
      idx = executions.findIndex(e => e.name === name && e.status === 'running' && e.args === args);
    }
    if (idx === -1) {
      idx = executions.findIndex(e => e.name === name && e.status === 'running');
    }
    if (idx === -1) {
      idx = executions.findIndex(e => e.name === name);
    }

    if (idx !== -1) {
      executions[idx] = {
        ...executions[idx],
        status,
        output,
        progress: 100,
      };
    } else {
      executions.push({
        id: `${name}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
        name,
        args,
        status,
        output,
        progress: 100,
      });
    }
    return { activeToolExecutions: executions };
  }),
  clearActiveToolExecutions: () => set({ activeToolExecutions: [] }),

  // Turn Review Request
  turnReviewRequest: null,
  setTurnReviewRequest: (req) => set({ turnReviewRequest: req }),

  currentPath: '/',
  fileItems: [],
  setExplorer: (path, items) => set({ currentPath: path, fileItems: items }),

  activeFile: null,
  setActiveFile: (file) => set({ activeFile: file, isFileEditable: false }), // Reset editable state when changing files
  updateActiveFileContent: (content) => set((state) => ({ activeFile: state.activeFile ? { ...state.activeFile, content } : null })),
  isFileEditable: false,
  setFileEditable: (val) => set({ isFileEditable: val }),

  isEditorFocused: false,
  setEditorFocused: (val) => set({ isEditorFocused: val }),

  isTerminalOpen: false,
  setTerminalOpen: (val) => set({ isTerminalOpen: val }),

  backgroundIntensity: 'subtle',
  setBackgroundIntensity: (intensity) => set({ backgroundIntensity: intensity }),

  activeTab: 'files',
  setActiveTab: (tab) => set({ activeTab: tab }),

  chatViewMode: 'timeline',
  setChatViewMode: (mode) => set({ chatViewMode: mode }),

  reasoningAccumulator: '',
  appendReasoningContent: (chunk) => set((state) => ({ reasoningAccumulator: state.reasoningAccumulator + chunk })),
  clearReasoning: () => set({ reasoningAccumulator: '' }),
  currentToolResults: [],
  addToolResult: (res) => set((state) => ({ currentToolResults: [...state.currentToolResults, res] })),
  clearToolResults: () => set({ currentToolResults: [] }),

  searchResults: [],
  isSearching: false,
  setSearchResults: (results) => set({ searchResults: results }),
  setSearching: (val) => set({ isSearching: val })
    }),
    {
      name: 'tempest-settings',
      partialize: (state) => ({
        isTerminalOpen: state.isTerminalOpen,
        backgroundIntensity: state.backgroundIntensity
      }) as any,
    }
  )
);
