# 🌪️ Tempest AI `v0.3.1` — "Cyber-Orchestrator"
**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer."

---

## 🚀 Launch Modes
Tempest can be started in several modes depending on your hardware and desired interface:

| Command | Mode | Backend |
| :--- | :--- | :--- |
| `./tempest_ai` | **TUI (Default)** | Ollama |
| `./tempest_ai --mlx` | **TUI** | MLX (Apple Silicon) |
| `./tempest_ai --cli` | **CLI** | Ollama |
| `./tempest_ai --mlx --cli` | **CLI** | MLX (Apple Silicon) |

---

## 🕹️ "Cyber-Orchestrator" TUI Controls

### 📂 Cyber-Explorer & Viewer
| Shortcut | Action |
| :--- | :--- |
| **`h` / `j` / `k` / `l`** | **Vim Navigation**: Navigate the workspace tree. |
| **`Backspace` / `h`** | **Up**: Go to parent directory. |
| **`Enter` / `l`** | **In**: Open folder or **View file** in Cyber-Viewer. |
| **`1` - `5`** | **Smart Action**: Dispatch the corresponding suggested action instantly. |
| **`e`** | **Edit**: Inject file context into chat with `[CONTEXT]` tag. |
| **`f`** | **Fix**: Instantly command the agent to fix the selected file. |
| **`r`** | **Refactor**: Trigger a refactor pulse for the selected file. |
| **`c`** | **Copy**: Copy the absolute path to the system clipboard. |
| **`d`** | **Delete**: Remove the file (Use with caution). |

### 🛰️ Global Navigation
| Shortcut | Action |
| :--- | :--- |
| **`Ctrl+P`** | **Mission Control Palette**: Fuzzy-search themes, models, and actions. |
| **`Ctrl+E`** | **Toggle Explorer**: Show/Hide the workspace browser. |
| **`Tab`** | **Focus Switch**: Cycle between Explorer, Chat, Viewer, and Reasoning. |
| **`Esc` / `q` / `x`** | **Close Viewer**: Dismiss the Cyber-Viewer (when focused). |
| **`Esc`** | **Interrupt**: Stop agent's current thought or execution loop. |

---

## 🦾 Smart Orchestrator Panel
When starting a session with no active messages, Tempest manifests the **Smart Orchestrator**:
1. **Selection Awareness**: It tracks your active file in the Explorer.
2. **Contextual Logic**: It generates 5 tailored actions based on file extension (e.g., Rust module vs. TOML config).
3. **Numeric Execution**: Pressing `1-5` while the explorer is focused dispatches the command immediately.

---

## ⚡ High-Velocity Mode & Safety

### The Safety Net (`/undo`)
Every modification made by Tempest triggers an **Automatic Checkpoint**. 
- **Atomic Backups**: Every file edit is backed up to a checkpoint manager before the first byte is written.
- **Instant Recovery**: If you don't like a change, type **`/undo`** (or select it from `Ctrl+P`). This reverts all modified files to their state immediately before the last tool execution.

### Safe Mode (`/safemode`)
- Toggle **Safe Mode** via `/safemode` or the `Ctrl+P` palette.
- When **ON**, Tempest will block and show you a **Unified Diff Preview** for every change, waiting for your explicit approval to proceed.

---

## 🔧 Configuration
Configuration is stored in `config.toml`. Key v0.3.1 fields:
- `tui_theme`: Your persistent aesthetic choice (Ocean, Mocha, Solarized).
- `mlx_model`: Default native engine preset for Apple Silicon.
- `planner_model` / `executor_model`: Ollama-tier logic configuration.
- `safe_mode`: Persistent toggle for execution safety.

---

> [!IMPORTANT]
> **Industrial Traversal**: Use **`h`** and **`l`** for deep-dive exploration. The Cyber-Explorer respects your `.gitignore` and hides heavy artifacts (target/, node_modules/) automatically. 🦾🚀✨
