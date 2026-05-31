# 🌪️ Tempest AI `v0.3.5` — "Command Center"

[![Download Latest Release](https://img.shields.io/badge/Download-Latest%20Release-blue?style=for-the-badge&logo=github)](https://github.com/7empest462/tempest_ai/releases/latest)

![License](https://img.shields.io/badge/license-Source--Available-blue?style=flat-square)
![GitHub Stars](https://img.shields.io/github/stars/7empest462/tempest_ai?style=flat-square&color=yellow)
![Rust Version](https://img.shields.io/badge/rust-1.95.0-orange?style=flat-square&logo=rust)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat-square&logo=typescript&logoColor=white)
![WebAssembly](https://img.shields.io/badge/wasm-654FF0?style=flat-square&logo=webassembly&logoColor=white)
![Engine](https://img.shields.io/badge/backend-MLX%20%7C%20Ollama%20%7C%20LMStudio%20%7C%20Bridge-blueviolet?style=flat-square)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Web-lightgrey?style=flat-square)

**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." With the v0.3.5 release, Tempest introduces **Google Gemini API Support** via the AI Bridge, **Smart 503 Fallback Routing**, **Semantic Compaction** for infinite context limits, and the **Agent Client Protocol (ACP)** for spawning parallel asynchronous sub-agents.

---

## 🕹️ Available Interfaces

- **🧠 Reasoning Monitor**: Live-streaming "Internal Monologue" stream showing the agent's thought process in real-time.
- **🚀 Mission Control Stepper**: Visual lifecycle tracking (Thinking → Planning → Executing) with pulse animations.
- **📦 Single-Binary Portable**: Web assets are embedded in the Rust binary — no external `dist` folder required.
- **💬 Neural Chat**: Conversational AI interface for autonomous engineering tasks
- **📂 File Explorer**: Full filesystem traversal with scrollable navigation and back-button support
- **📄 Code Deck**: Syntax-highlighted file viewer with inline editing and persistent save-to-disk
- **🖥️ PTY Terminal**: Real `zsh` session via `portable-pty` + `xterm.js` with auto-resize
- **🔍 Code Search**: Project-wide `grep` search — click results to open files in the editor
- **📊 Live Telemetry**: Real-time CPU, GPU (Metal/Linux), and RAM metrics in the header bar
- **🔀 Backend Switcher**: Toggle between MLX, Ollama, AI Bridge, and LM Studio

```bash
# Launch the Standalone Web Command Center
./target/release/tempest_ai --web --mlx        # Apple Silicon (Metal + Neural Engine)
./target/release/tempest_ai --web              # Default: Ollama backend
./target/release/tempest_ai --web --lmstudio   # LM Studio (local OpenAI-compatible API)
./target/release/tempest_ai --gemini           # Google Gemini API
```

### 📟 "Cyber-Orchestrator" TUI
An industrial, high-fidelity terminal dashboard for full-screen autonomous workflows.

- **🦾 Smart Orchestrator Panel**: Context-aware suggestions based on your selected file
- **⚡ High-Velocity Shortcuts**: Numeric keys `1-5` for instant action dispatch
- **📂 Cyber-Explorer (Ctrl+E)**: Vim-style workspace navigation (`h/j/k/l`)
- **📄 Cyber-Viewer**: High-fidelity code observation pane
- **📊 Mission Control Pulse**: Boxed telemetry sparklines for CPU, GPU, and TPS

```bash
./target/release/tempest_ai       # Default: Ollama Engine
./target/release/tempest_ai --mlx # Premium: MLX Apple Silicon
```

### 💻 VS Code Sidebar
A Vue 3-powered engineering dashboard with glassmorphism design.

- **⚡ Smart Toolbar**: One-click actions — Fix, Explain, Refactor, Comment
- **🧠 Editor Awareness**: Explicit `[EDITOR GROUND TRUTH]` injection for accuracy

### 🖥️ Standard CLI
Lightweight, direct command-line interface for rapid tasks and piping.

```bash
./target/release/tempest_ai --cli
./target/release/tempest_ai --mlx --cli
```

---

## ⚙️ Multi-Backend Architecture

Tempest supports four inference backends, selectable at launch:

| Backend | Flag | Hardware | Description |
|:--------|:-----|:---------|:------------|
| **🍏 MLX** | `--mlx` | Apple Silicon M1–M4 | Native Metal GPU + Neural Engine, ultra-low latency |
| **🐋 Ollama** | *(default)* | Any | Cross-platform local inference via Ollama API |
| **⚡ AI Bridge** | `--bridge` | Any | Remote providers, provider-agnostic API bridging |
| **🔬 LM Studio** | `--lmstudio` | Any | Local LM Studio integration |

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────┐
│              Tempest AI (Rust Core)              │
│  Agent → Inference → Tools → Checkpoint → TUI   │
├─────────────────────────┬───────────────────────┤
│   Nexus WebSocket       │   tempest-wasm        │
│   (axum + portable-pty) │   (wasm-bindgen)      │
├─────────────────────────┼───────────────────────┤
│   Web Command Center    │   VS Code Extension   │
│   (Vite + xterm.js)     │   (Vue 3)             │
└─────────────────────────┴───────────────────────┘
```

### Key Subsystems
- **Agent Orchestration** (`src/agent.rs`): State machine driving the turn lifecycle — Thinking → PendingTools → ExecutingTools → Done
- **Nexus WebSocket Engine** (`src/nexus.rs`): Real-time bridge serving the Web Command Center — telemetry, file I/O, PTY terminal, and code search over a single WebSocket connection
- **Inference Backends** (`src/inference.rs`): Unified trait abstracting MLX, Ollama, and AI Bridge
- **Tool System** (`src/tools/`): Extensible tool registry with parallel execution and automatic checkpointing
- **Sentinel Guards** (`src/sentinel.rs`): Runtime safety — loop detection, context pressure, hallucination checks
- **Context Manager** (`src/context_manager.rs`): Token-aware history compaction via sub-model summarization
- **Checkpoint System** (`src/checkpoint.rs`): Reversible file-change batches with diff preview and `/undo` support
- **MCP Integration** (`src/mcp.rs`): Dynamic tool discovery from external MCP servers

---

## 🚀 Key Abilities
- **🔌 MCP Protocol Support**: Native integration with any Model Context Protocol server
- **⚡ Parallel Tool Execution**: Independent tool calls dispatched concurrently
- **⏪ Multi-Level Undo**: Automatic file snapshots before every modification — `/undo` to revert
- **📊 Real-Time Telemetry**: CPU, GPU (Metal/Linux), and RAM monitoring via `tempest-monitor`
- **📦 Standalone Portable**: Web Command Center assets embedded via `include_dir` for single-file deployment
- **🛡️ Safe Mode**: Visual diff previews for all proposed changes during the approval phase
- **🧠 MLX Stabilization**: Hard hardware RAM capping (90% ceiling) and Prefix Cache eviction (16 sequences) for unbounded uptime on Apple Silicon
- **📡 Port Negotiation**: Automatic upward port scanning for Nexus (8080+) and Metrics (7777+) to prevent startup collisions
- **📦 Unified Library**: Core engine as a standalone Rust crate for WASM, VS Code, and CLI targets

---

## 🛠️ Quick Start

### Prerequisites
- **Rust** 1.95+ with `wasm32-unknown-unknown` target
- **Node.js** 18+ (for the Web Command Center)
- **wasm-pack** (`cargo install wasm-pack`)
- One of: Ollama, MLX-compatible Mac, or LM Studio

### 1. Clone & Build
```bash
git clone https://github.com/7empest462/tempest_ai.git
cd tempest_ai
./tempest.sh build   # Builds WASM, Vite frontend, and Rust backend
```

### 2. Launch

```bash
# Web Command Center (Standalone)
./target/release/tempest_ai --web --mlx

# TUI Mode
./target/release/tempest_ai --mlx

# CLI Mode
./target/release/tempest_ai --cli

# MCP Server (JSON-RPC over stdio)
./target/release/tempest_ai --mcp_server
```

### 3. Access the Web Command Center
Open **`http://localhost:8080`** in your browser after launching with `./tempest.sh web`.

---

## 📦 Project Structure

```
tempest_ai/
├── src/                  # Rust core — agent, inference, tools, nexus
│   ├── agent.rs          # Orchestration state machine
│   ├── inference.rs      # Backend abstraction (MLX, Ollama, Bridge)
│   ├── nexus.rs          # WebSocket engine (telemetry, PTY, file I/O)
│   ├── tools/            # Tool implementations
│   ├── sentinel.rs       # Runtime safety guards
│   ├── checkpoint.rs     # Reversible file changes
│   └── mcp.rs            # MCP client integration
├── tempest-wasm/         # WASM crate (wasm-bindgen dashboard)
├── tempest-web/          # Vite + TypeScript frontend
│   ├── src/main.ts       # xterm.js, search, editor, chat logic
│   ├── src/style.css     # Glassmorphic design system
│   └── index.html        # Command Center layout
├── vscode-tempest/       # VS Code extension (Vue 3)
├── tempest.sh            # Unified build/launch orchestrator
├── config.toml           # Model routing and sampling defaults
└── MANUAL.md             # TUI keyboard shortcuts reference
```

---

## 🔧 Configuration

Configuration is loaded from `config.toml` (lookup order: `--config` flag → `./config.toml` → `~/.config/tempest_ai/config.toml`).

Key fields:
- `mlx_model` — Default MLX model preset for Apple Silicon
- `planner_model` / `executor_model` — Model routing for planning vs. execution phases
- `safe_mode` — Persistent toggle for execution safety (diff approval before changes)
- `tui_theme` — Persistent TUI aesthetic (Ocean, Mocha, Solarized)
- `nexus_port` / `metrics_port` — Custom overrides for web and telemetry services

---

## ⚖️ License
Tempest AI is released under the **Tempest AI Source-Available License**.
- **Free for Personal, Educational, and Internal Business use.**
- **All commercial rights are exclusively reserved by Robert Simens.**

---

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
