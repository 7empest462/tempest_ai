# 🌪️ Tempest AI `v0.3.2` — "Cyber-Orchestrator"

![License](https://img.shields.io/badge/license-Source--Available-blue?style=flat-square)
![GitHub Stars](https://img.shields.io/github/stars/7empest462/tempest_ai?style=flat-square&color=yellow)
![Rust Version](https://img.shields.io/badge/rust-1.95.0-orange?style=flat-square&logo=rust)
![TypeScript](https://img.shields.io/badge/typescript-%23007ACC.svg?style=flat-square&logo=typescript&logoColor=white)
![Engine](https://img.shields.io/badge/backend-MLX%20%7C%20Ollama%20%7C%20LMStudio%20%7C%20Bridge-blueviolet?style=flat-square)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20WASM-lightgrey?style=flat-square)

**The Hardware-Aware, Local-Inference Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." With the v0.3.2 release, Tempest evolves into a **Library/Binary Hybrid Architecture**, separating its core "Frontal Lobe" reasoning from the interface layer to deliver unparalleled stability and multi-platform consistency.

---

## 🕹️ Available Interfaces
Tempest AI is built for versatility, offering three distinct ways to interact:

### 📟 "Cyber-Orchestrator" TUI
An industrial, high-fidelity terminal dashboard for full-screen autonomous workflows.
- **🦾 Smart Orchestrator Panel**: A dynamic welcome dashboard with **Context-Aware Suggestions** based on your currently selected file.
- **⚡ High-Velocity Shortcuts**: Use numeric keys **`1-5`** in the explorer to instantly dispatch suggested commands.
- **📂 Cyber-Explorer (Ctrl+E)**: Real-time workspace navigation with **Vim-style navigation** (`h/j/k/l`).
- **📄 Cyber-Viewer**: High-fidelity code observation pane. Press **`Enter`** or **`l`** in the explorer to manifest.
- **📊 Mission Control Pulse**: Boxed telemetry sparklines for **CPU**, **GPU**, and **TPS Velocity**.

### 💻 VS Code Sidebar
The premium engineering experience. A modern, **Vue 3-powered** dashboard with a sleek glassmorphism design.
- **⚡ Smart Toolbar**: One-click engineering actions—`Fix`, `Explain`, `Refactor`, and `Comment`.
- **🧠 Hardened Editor Awareness**: Explicit `[EDITOR GROUND TRUTH]` injection for high-fidelity accuracy.

### 🖥️ Standard CLI
A lightweight, direct command-line interface for rapid tasks and piping workflows. Use the `--cli` flag to activate.

---

## ⚙️ Triple Backend Architecture
Tempest AI v0.3.2 utilizes a sophisticated **Triple Backend System** to ensure high-performance inference regardless of your environment:

1. **🍏 MLX Engine (Premium)**: Built specifically for Apple Silicon (M1-M4). Utilizes the Metal GPU and Neural Engine for ultra-low latency inference. Activate with `--mlx`.
2. **🐋 Ollama Engine (Standard)**: Local cross-platform support for Linux, Windows, and Intel Macs. Connects seamlessly to any local Ollama model.
3. **⚡ AI Bridge (Versatile)**: Support for remote providers, LM Studio integration, and provider-agnostic API bridging, ensuring you're never locked into a single stack.

---

## 🚀 Key Abilities
- **🔌 MCP Protocol Support**: Native integration with any Model Context Protocol server for dynamic tool expansion.
- **⚡ Parallel Tool Execution**: High-velocity pipeline executes independent tool calls in parallel.
- **⏪ Multi-Level Undo**: Automatic file snapshots before every modification. Use `/undo` to revert.
- **📊 Real-Time Telemetry**: High-frequency metrics collector monitoring **TPS** and system vitals via a dedicated library loop.

---

## 🏗️ The "Cyber-Library" Core
- **🛡️ Programmatic Safety**: All system-modifying actions are blocked until an explicit **Implementation Plan** is approved.
- **Visual Diff Previews**: High-fidelity, colorized diffs are generated for all proposed changes during the approval phase.
- **⚡ DeepSeek Normalizer**: Automatically repacks reasoning model arguments into strict schema objects to ensure 100% tool-call reliability.
- **📦 Unified Library**: The core engine is now a standalone Rust crate (`tempest_ai`), enabling stable integration for WASM, VS Code, and CLI targets.

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
# Start the v0.3.2 TUI (Default: Ollama Engine)
./target/release/tempest_ai

# Start the v0.3.2 TUI (Premium: MLX Apple Silicon Engine)
./target/release/tempest_ai --mlx

# Start the Standard CLI
./target/release/tempest_ai --cli
```

---

## ⚖️ License
Tempest AI is released under the **Tempest AI Source-Available License**.
- **Free for Personal, Educational, and Internal Business use.**
- **All commercial rights are exclusively reserved by Robert Simens.**

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
