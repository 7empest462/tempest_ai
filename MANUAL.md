# 🌪️ Tempest AI `v0.3.0` — "Cyber-Orchestrator"
**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat-wrappers, Tempest is a **Stateful Intelligence** that operates with a hardened "Frontal Lobe" architecture—enforcing programmatic boundaries, real-time situational awareness, and a disciplined Planning/Execution lifecycle.

---

## 🕹️ "Cyber-Orchestrator" TUI Controls

The TUI is designed as a high-fidelity Mission Control dashboard. Use the following shortcuts to navigate the environment:

| Shortcut | Action |
| :--- | :--- |
| **`Ctrl+P`** | **Mission Control Palette**: Fuzzy-searchable hub for model presets, themes, and system actions. |
| **`Ctrl+E`** | **Interactive Explorer**: Toggle the workspace file browser. |
| **`Ctrl+C`** | **Emergency Exit**: Gracefully terminate the engine and unload models. |
| **`Tab`** | **Focus Switch**: Cycle focus between Chat, Reasoning, and Explorer panes. |
| **`Esc`** | **Interruption**: Stop the agent's current thought or execution loop. |
| **`Mouse Wheel`**| **Scroll**: Direct interaction with Chat and Reasoning buffers. |

---

## 📊 Mission Control HUD

The dashboard provides real-time observability into the agent's performance and system health:

### 1. Telemetry Pulse
- **CPU Load**: Real-time rolling history of all core activity.
- **GPU Activity**: High-fidelity Metal/Vulkan load tracking.
- **TPS (Tokens Per Second)**: Live generation velocity history.

### 2. Context HUD
- **Usage Ratio**: Displays current tokens vs. total capacity (e.g., `12k / 32k`).
- **Sentinel Fleet**: Real-time status of background monitors (Security, Syntax, and Rules).

---

## 🎨 Persistent Aesthetic Engine

Tempest AI supports professional-grade syntax highlighting for its Reasoning Trace and code blocks.
- **Hot-Swapping**: Use `Ctrl+P` and search "Theme" to cycle through **Ocean**, **Mocha**, **Eighties**, and **Solarized**.
- **Persistence**: Your theme selection is automatically committed to `config.toml` and restored on every boot.

---

## ⚡ High-Velocity Mode & Safety

Tempest AI operates in **High-Velocity Mode** by default. It will perform modifications without asking for permission, allowing for rapid-fire iteration.

### The Safety Net (`/undo`)
Every modification made by Tempest triggers an **Automatic Checkpoint**. 
- If you don't like a change, simply wait for the turn to end and type **`/undo`** (or select it from `Ctrl+P`).
- This will revert all modified files to their state immediately before the last tool execution.

### Safe Mode (`/safemode`)
If you are working on sensitive production code:
- Toggle **Safe Mode** via `/safemode` or the `Ctrl+P` palette.
- When **ON**, Tempest will block and show you a **Unified Diff Preview** for every change, waiting for your approval to proceed.

---

## 🛠️ Integrated Command Hub

All system actions can be accessed via either **Slash Commands** or the **Fuzzy Palette (`Ctrl+P`)**:

| Command | Palette Entry | Description |
| :--- | :--- | :--- |
| `/help` | `Help: Manual` | Display this guide. |
| `/undo` | `System: Undo` | Revert the last file modification. |
| `/switch` | `Hot-Swap: <Model>` | Change the inference engine preset. |
| `/safemode`| `Toggle: Safe Mode`| Switch between Velocity and Safety modes. |
| `/clear` | `System: Clear` | Wipe conversation history and reset context. |

---

## 🔧 Configuration
Configuration is stored in `config.toml` (locally) or `~/.config/tempest_ai/config.toml`. Key v0.3.0 fields:
- `tui_theme`: Your persistent aesthetic choice.
- `mlx_model`: Default native engine preset.
- `planner_model` / `executor_model`: Ollama-tier logic configuration.

---

> [!TIP]
> **Pro Tip**: Use the **Explorer (`Ctrl+E`)** to select specific files for context injection. Once focused on a file, pressing `Enter` will append its path to your input buffer with the `[CONTEXT]` tag, ensuring the agent has high-fidelity access to your ground truth. 🦾🚀✨
