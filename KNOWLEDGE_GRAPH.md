# 🌪️ Tempest AI: Knowledge Graph (v0.3.5 "Cyber-Orchestrator")

This document visualizes the internal architecture and data flow of the Tempest AI engine, including the new AI Bridge, Sub-agent operations, and Context Manager.

## 🧠 System Architecture (Mermaid)

```mermaid
graph TD
    %% Entry & Configuration
    User((USER)) -->|Input| TUI[src/tui.rs]
    Main[src/main.rs: Binary] -->|Load Config| AppConfig[tempest_ai::AppConfig]
    Main -->|Spawn| TUI
    Main -->|Initialize| Agent[tempest_ai::Agent]
    
    %% Core Orchestration & Command Hub
    TUI -->|UserRequest| Agent
    TUI -->|Ctrl+P| Palette[Command Palette]
    Palette -->|Hot-Swap| Inference[tempest_ai::inference::Backend]
    
    %% TUI & Explorer
    TUI -->|Ctrl+E| Explorer[Cyber-Explorer]
    Explorer -->|Navigate| Viewer[Cyber-Viewer]
    
    %% Inference Layer
    Agent -->|State: Thinking| Inference
    Inference -->|MLX Backend| MLX[Apple Metal GPU]
    Inference -->|Ollama Backend| Ollama[System Resources]
    Inference -->|AI Bridge| Bridge[src/ai_bridge.rs]
    Bridge -->|API Request| Gemini[Google Gemini / Cloud]
    Bridge -->|API Request| OpenAI[OpenAI API]
    
    %% Execution Loop & Safety
    Inference -->|Raw Tokens| Normalizer[tempest_ai::inference::Normalizer]
    Normalizer -->|Native ToolCalls| Agent
    Agent -->|State: PendingTools| Checkpoint[tempest_ai::checkpoint::CheckpointManager]
    Checkpoint -->|Backup Workspace| Storage[(.tempest/backups)]
    Checkpoint -->|Dispatch| Tools[tempest_ai::tools::*]
    Tools -->|Success/Failure| Agent
    
    %% Subsystems & Scalability
    Tools -->|Agent Client Protocol (ACP)| AgentOps[src/tools/agent_ops.rs: Sub-agents]
    Tools -->|Dynamic Tools| MCP[src/mcp.rs: JSON-RPC Servers]
    AgentOps -->|Spawn Async| SubAgent[tempest_ai::Agent]
    
    %% Memory & Context Protection
    Agent -->|Query| Memory[tempest_ai::memory::MemoryStore]
    Memory -->|Vector Search| Brain[(vector_brain.json)]
    Agent -->|Token Pressure| Context[src/context_manager.rs]
    Context -->|Semantic Compaction| History[(history.json)]
    Agent -->|Verify| Sentinel[tempest_ai::sentinel::Sentinel]
```

## 🛰️ Key Interaction Chains (v0.3.5)

### 1. The Inference Routing
The `Backend` enum in `src/inference.rs` acts as the router for model execution. Depending on configuration (`--gemini`, `--mlx`, `--bridge`), Tempest AI will hot-swap the underlying LLM provider mid-flight. The new `AI Bridge` handles asynchronous streaming and REST requests to API services like Google Gemini.

### 2. Context & Memory Handling
When the main thread detects token saturation, the `ContextManager` steps in. It intercepts the `history.json` payload and uses a secondary LLM pipeline to execute a **semantic compaction**, dropping noisy tokens while preserving intent, allowing near-infinite task lifespan.

### 3. Agent Scalability & Delegation
Through the Agent Client Protocol (ACP), the primary Agent can use `agent_ops.rs` to spawn recursive `Sub-Agent` instances. These sub-agents run in isolated asynchronous threads, complete designated sub-tasks, and report back via ACP, allowing the primary agent to conquer massive projects in parallel.
