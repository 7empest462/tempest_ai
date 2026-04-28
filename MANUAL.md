# 🌪️ Tempest AI — User Manual

Tempest AI is an autonomous agentic coding assistant designed for high-velocity software engineering. It operates on a "Dual-Engine" architecture, separating high-level planning from precision execution.

---

## 🚀 Getting Started

### Backends
Tempest supports two primary inference backends:
- **Ollama (Default)**: Best for cross-platform compatibility and larger models (30B+).
- **MLX (Apple Silicon)**: Native integration for Mac (M1/M2/M3/M4). Uses the Neural Engine and GPU for massive speed boosts.
  - *Usage*: `tempest_ai --mlx --quant Q4_K_M`

---

## ⚡ High-Velocity Mode & Safety

Tempest AI operates in **High-Velocity Mode** by default. It will perform modifications without asking for permission, allowing for rapid-fire iteration.

### The Safety Net (`/undo`)
Every modification made by Tempest triggers an **Automatic Checkpoint**. 
- If you don't like a change, simply wait for the turn to end and type **`/undo`**.
- This will revert all modified files to their state immediately before the last tool execution.

### Safe Mode (`/safemode`)
If you are working on sensitive production code or mission-critical files:
- Type **`/safemode`** to toggle the **Approval Gate**.
- When Safe Mode is **ON**, Tempest will block and show you a **Unified Diff Preview** for every change, waiting for your `[ENTER]` to proceed.

---

## 🛠️ Slash Commands

| Command | Description |
| :--- | :--- |
| `/help` | Display this manual. |
| `/undo` | Revert the last batch of file modifications. |
| `/safemode` | Toggle blocking approvals on/off. |
| `/switch <model>` | Hot-swap the model without restarting. |
| `/clear` | Clear the current conversation history (reset context). |
| `/exit` | Gracefully shut down Tempest. |

---

## 🧠 Core Systems

### 1. Planning vs. Execution
- **Planning Phase**: The agent analyzes your request, reads files, and maps out a solution.
- **Execution Phase**: The agent dispatches specialized sub-agents to apply the code changes.
- **Parallelism**: Tempest executes non-modifying tools (like reading files or searching) in parallel to minimize latency.

### 2. Repetition Sentinel
If the agent enters a "loop" (e.g., trying the same failed command repeatedly), the Sentinel will automatically intervene, block the execution, and force the agent to rethink its strategy.

### 3. Context Management
Tempest uses a sliding window context manager. It automatically compresses old reasoning steps while keeping the latest file contents and tool results in focus to prevent "hallucination plateau."

---

## 🔧 Configuration
Configuration is stored in `~/.config/tempest_ai/config.toml`. You can customize:
- `planner_model` / `executor_model`
- `temp_planning` / `temp_execution`
- `ctx_limit` (Context size)

---

> [!TIP]
> **Pro Tip**: Use `/undo` early and often. Don't be afraid to let the agent experiment in High-Velocity mode; the checkpoint system is designed to handle the mess!
