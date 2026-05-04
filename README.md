# 🌪️ Tempest AI `v0.3.1` — "Cyber-Orchestrator"
**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat-wrappers, Tempest is a **Stateful Intelligence** that operates with a hardened "Frontal Lobe" architecture—enforcing programmatic boundaries, real-time situational awareness, and a disciplined Planning/Execution lifecycle.

---

## 🕹️ Available Interfaces
Tempest AI is built for versatility, offering three distinct ways to interact:

### 📟 "Cyber-Orchestrator" TUI (v0.3.1 Update)
An industrial, high-fidelity terminal dashboard for full-screen autonomous workflows.
- **🦾 Smart Orchestrator Panel**: A dynamic welcome dashboard with **Context-Aware Suggestions** based on your currently selected file.
- **⚡ High-Velocity Shortcuts**: Use numeric keys **`1-5`** in the explorer to instantly dispatch suggested commands.
- **📂 Cyber-Explorer (Ctrl+E)**: Real-time workspace navigation with **Vim-style navigation** (`h/j/k/l`).
- **📄 Cyber-Viewer**: High-fidelity code observation pane. Press **`Enter`** or **`l`** in the explorer to manifest.
- **📊 Mission Control Pulse**: Boxed telemetry sparklines for **CPU**, **GPU**, and **TPS**.
- **⌨️ Mission Control Palette (Ctrl+P)**: Fuzzy-searchable hub for themes, models, and protocols.

### 💻 VS Code Sidebar
The premium engineering experience. A modern, **Vue 3-powered** dashboard with a sleek glassmorphism design.
- **⚡ Smart Toolbar**: One-click engineering actions—`Fix`, `Explain`, `Refactor`, and `Comment`.
- **🧠 Hardened Editor Awareness**: Explicit `[EDITOR GROUND TRUTH]` injection for high-fidelity accuracy.

### 🖥️ Standard CLI
A lightweight, direct command-line interface for rapid tasks and piping workflows. Use the `--cli` flag to activate.

---

## ⚙️ Engines & Hardware
Tempest AI is designed to be hardware-aware, utilizing the best available local resources:

- **🍏 MLX Engine (Premium)**: Built specifically for Apple Silicon (M1-M4). Utilizes the Metal GPU and Neural Engine. Activate with the `--mlx` flag.
- **🐋 Ollama Engine (Default)**: Cross-platform support for Linux, Windows, and Intel Macs. Connects to any Ollama model.
- **🧠 Hybrid Awareness**: Automatically detects hardware and scales the context window and reasoning loops to match available VRAM.

---

## 🚀 Key Abilities
- **🔌 MCP Protocol Support**: Native integration with any Model Context Protocol server.
- **⚡ Parallel Tool Execution**: High-velocity pipeline executes independent tool calls in parallel.
- **⏪ Multi-Level Undo**: Automatic file snapshots before every modification. Use `/undo` to revert.
- **🧪 Competency Tracking**: Monitors tool success rates; enters "Self-Reflective" mode upon repeated failures.

---

## 🏗️ The "Frontal Lobe" Architecture
- **🛡️ Programmatic Safety**: All system-modifying actions are blocked until an explicit **Implementation Plan** is approved.
- **Visual Diff Previews**: High-fidelity, colorized diffs are generated for all proposed changes during the approval phase.
- **⚡ DeepSeek Normalizer**: Automatically repacks reasoning model arguments into strict schema objects to prevent tool-call failures.

---

## 🛠️ Quick Start

### 1. Build the Backend
```bash
git clone https://github.com/7empest462/tempest_ai.git
cd tempest_ai
cargo build --release
```

### 2. Launch the Orchestrator
```bash
# Start the v0.3.1 TUI (Default: Ollama Engine)
./target/release/tempest_ai

# Start the v0.3.1 TUI (Premium: MLX Apple Silicon Engine)
./target/release/tempest_ai --mlx

# Start the Standard CLI (Ollama)
./target/release/tempest_ai --cli

# Start the Standard CLI (MLX)
./target/release/tempest_ai --mlx --cli
```

---

## ⚖️ License
Tempest AI is released under the **Tempest AI Source-Available License**.
- **Free for Personal, Educational, and Internal Business use.**
- **All commercial rights are exclusively reserved by Robert Simens.**

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
