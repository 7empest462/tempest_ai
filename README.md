# 🌪️ Tempest AI `v0.3.0` — "Cyber-Orchestrator"
**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat-wrappers, Tempest is a **Stateful Intelligence** that operates with a hardened "Frontal Lobe" architecture—enforcing programmatic boundaries, real-time situational awareness, and a disciplined Planning/Execution lifecycle.

---

## 🕹️ Available Interfaces
Tempest AI is built for versatility, offering three distinct ways to interact:

### 💻 VS Code Sidebar
The premium engineering experience. A modern, **Vue 3-powered** dashboard with a sleek glassmorphism design.
- **⚡ Smart Toolbar**: Context-aware quick actions—`Fix`, `Explain`, `Refactor`, and `Comment`—that automatically snapshot your active file and cursor position for instant, one-click engineering.
- **🧠 Hardened Editor Awareness**: Explicit `[EDITOR GROUND TRUTH]` injection ensures the agent prioritizes your active file path and code selection over internal hallucinations.
- **🌀 Real-Time Feedback Loop**: Watch the "Storm" in motion with a pulsing thought HUD, a blinking streaming cursor, and a dynamic status bar that tracks the agent's phase (**THINKING**, **ANALYZING**, **LOADING**) in real-time.

### 📟 "Cyber-Orchestrator" TUI (v0.3.0 Update)
An industrial, high-fidelity terminal dashboard for full-screen autonomous workflows.
- **📊 Mission Control Pulse**: Boxed, real-time telemetry sparklines for **CPU Load**, **GPU Activity (Metal)**, and **TPS (Token Generation Velocity)**.
- **⌨️ Fuzzy Command Palette (Ctrl+P)**: A global, searchable hub to hot-swap models, toggle **Safe Mode**, or manage the **Persistent Aesthetic Engine**.
- **🎨 Persistent Aesthetic Engine**: Real-time hot-swapping between premium syntax themes (**Ocean**, **Mocha**, **Eighties**, **Solarized**) with automatic memory—selection is saved to `config.toml` and restored on boot.
- **🧠 Advanced Context HUD**: Detailed usage ratios (e.g., `12k / 32k`) with a synchronized progress bar and **Sentinel Fleet** status.
- **Synchronized Reasoning**: Dynamic ASCII spinners and real-time `<think>` block parsing show exactly what the model is doing.
- **Slash Commands**:
  - `/help` — Show the full command manual.
  - `/undo` — Revert the last file modification.
  - `/switch <name>` — Hot-swap the MLX inference engine preset.
  - `/clear` — Wipe the conversation history.

### 🖥️ Standard CLI
A lightweight, direct command-line interface for rapid tasks, scriptable interactions, and piping workflows.

---

## ⚙️ Engines & Hardware
Tempest AI is designed to be hardware-agnostic while still squeezing every drop of performance out of your local machine.

- **🍏 MLX Engine (Premium)**: Built specifically for Apple Silicon (M1/M2/M3/M4). Utilizes the Metal GPU and Neural Engine for high-speed, unified-memory inference.
- **🐋 Ollama Engine (Cross-Platform)**: Fully supports **Linux, Windows, and Intel Macs**. Connect to any model in the Ollama library (DeepSeek-R1, Llama 3, Qwen, etc.) for a flexible, local engineering experience.
- **🧠 Hybrid Awareness**: Tempest automatically detects your hardware and scales its context window and reasoning loops to match your available VRAM.

---

## 🚀 Key Abilities
- **🔌 MCP Protocol Support**: Native integration with the Model Context Protocol. Connect Tempest to any MCP server (Git, SQLite, Jira, Slack) to expand its toolset dynamically.
- **⚡ Parallel Tool Execution**: High-velocity pipeline that executes independent tool calls (e.g., reading 5 different files) in parallel using `tokio` tasks for massive speed gains.
- **⏪ Multi-Level Undo**: Automatic file snapshots before every modification. Revert changes across the entire workspace with a single command.
- **🧠 Persistent Memory & Context Management**: Background summarization is offloaded to a secondary micro-model, preserving the primary reasoning window. Intelligent session restoration injects previous progress into the start of a new run.
- **🧪 Competency Tracking**: The agent monitors its own success/failure rates. If a tool fails repeatedly, Tempest enters a "Self-Reflective" state, injecting a Competency Warning that forces the model to analyze the failure before trying again.

---

## 🏗️ The "Frontal Lobe" Architecture
Tempest is hardened with a sophisticated agency management layer that prevents hallucinations and unauthorized executions.

### 🛡️ Programmatic Safety Gates & Checkpoints
- **Approval-First Execution**: All system-modifying actions are strictly blocked until an explicit **Implementation Plan** is approved.
- **Automatic Snapshots**: Every file edit is backed up to a checkpoint manager before the first byte is written.
- **Visual Diff Previews**: High-fidelity, colorized diffs are generated for all proposed changes during the approval phase.

### 🛰️ State-Injected Turns
Every reasoning turn begins with a high-priority **Situational Report** injected directly into the context:
- **Mode Awareness**: Explicitly informs the model if it is in **Planning** (Read-only research) or **Execution** (Active engineering) mode.
- **Approval Tracking**: Injects the current verification status of the proposed plan.

### 🧪 Competency HUD & Tool Stats
- **Performance Tracking**: Successful vs. failed tool calls are tallied in a thread-safe registry.
- **Self-Reflective Warnings**: If a tool fails repeatedly, the system injects a **Competency Warning**, forcing the model to stop and analyze the failure.

---

## 🛠️ The Tempest Toolbox (60+ Tools)
Tempest ships with a massive suite of native capabilities, now expandable via MCP.

### 📂 File & AST Suite
- **Navigation**: `project_atlas`, `tree`, `list_dir`, `search_files`.
- **AST-Aware**: `ast_outline`, `ast_edit` (Tree-sitter powered for Rust, Python, JavaScript, TypeScript).
- **IO**: `read_file`, `write_file`, `patch_file`, `find_replace`.

### 💻 System & Network
- **Admin**: `run_command`, `service_manager`, `process_control`.
- **Network**: `network_scan`, `port_discovery`.

### 🔌 External Ecosystem (via MCP)
- **Dynamic Discovery**: Connect to any external MCP server via `config.toml`.
- **First-Class Tools**: MCP tools are proxied and registered dynamically, appearing to the LLM exactly like native Rust tools.

### ⚡ DeepSeek Normalizer
- **Argument Repacking**: Automatically detects and repacks root-level flat arguments generated by reasoning models into strict schema objects, ensuring tool calls never fail due to formatting quirks.

---

## 🚀 Quick Start

### 1. Build the Backend
```bash
git clone https://github.com/7empest462/tempest_ai.git
cd tempest_ai
cargo build --release
```

### 2. Run via CLI or TUI
```bash
# Standard CLI mode
./target/release/tempest_ai

# Professional TUI mode
./target/release/tempest_ai --tui
```

### 3. Launch the VS Code Extension
1. Open the `vscode-tempest` folder in VS Code.
2. Run `npm install` and `npm run compile`.
3. Press **F5** to launch a new "Extension Development Host" window.
4. Click the 🌪️ icon in the Activity Bar to open Tempest.

---

## ⚖️ License
Tempest AI is released under the **Tempest AI Source-Available License**.
- **Free for Personal, Educational, and Internal Business use.**
- **All commercial rights (selling, SaaS, managed services, etc.) are exclusively reserved by Robert Simens.**
- See the [LICENSE](LICENSE) file for full details.

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
