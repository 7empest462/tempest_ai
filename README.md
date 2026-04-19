# 🌪️ Tempest AI (Project Smart-Brain)
**The Hardware-Aware, Native-Schema Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat aliases, Tempest is a **Stateful Intelligence** that operates with a hardened "Frontal Lobe" architecture—enforcing programmatic boundaries, real-time situational awareness, and a disciplined Planning/Execution lifecycle.

---

## 🧠 The "Frontal Lobe" Architecture
Tempest has been hardened with a sophisticated agency management layer that prevents hallucinations and unauthorized executions.

### 🛡️ 1. Programmatic Safety Gates
All system-modifying actions (File I/O, Command Execution, Git Commits) are strictly blocked at the hardware level until an explicit **Implementation Plan** has been approved by the user. 
- **Modifying Layer Protection**: Tools like `write_file` or `run_command` return a `BLOCKED` status in draft mode.
- **Planning Mandate**: The agent is programmatically forced to provide a `# PROPOSED PLAN` summary before it can trigger the `ask_user` tool for execution approval.

### 🛰️ 2. State-Injected Turns
Tempest is never "lost" in a long thread. Every reasoning turn begins with a high-priority **Situational Report** injected directly into the context:
- **Mode Awareness**: Explicitly informs the model if it is in Planning (Read-only) or Execution mode.
- **Approval Tracking**: Injects the current verification status of the proposed plan.

### 🧪 3. Competency HUD & Tool Stats
The agent monitors its own hardware and software "Win/Loss" record. 
- **Performance Tracking**: Successful vs. failed tool calls are tallied in a thread-safe registry.
- **Self-Reflective Warnings**: If a tool fails repeatedly, the system injects a **Competency Warning** at the start of the next turn, forcing the model to stop, analyze the failures, and verify its assumptions.

### 🌡️ 4. Dynamic Temperature Governance
Tempest dynamically shifts its internal reasoning "thermal" state:
- **Planning Mode (0.1 Temp)**: Locked for maximum logical rigor and architectural discipline.
- **Execution Mode (0.4 Temp)**: Optimized for creative problem-solving and implementation agility.

---

## 🚀 "Native-Engine" Capabilities

### ⚡ 1. Native Tool-Calling Architecture
Tempest is powered by the **`ollama-rs` 0.3.4** typed tool-calling framework. 
- **Strongly Typed**: Every tool is defined using `schemars` JSON schemas, eliminating brittle regex-based Markdown parsing.
- **Architectural Fallbacks**: Hallucinated tool calls are intercepted and replaced with **Corrective Advisories**, guiding the model back to the valid Tool Schema.
- **Multi-Turn Chaining**: Supports multiple sequential tool executions in a single reasoning step.

### 🧠 2. Categorized Long-Term Memory & Dual-Model Compaction
Tempest features a persistent SQLite-backed **Conceptual Brain** and a background-managed context window.
- **Micro-Model Compaction**: Background summarization is offloaded to a secondary model (e.g., `llama3.2:1b`), preserving context window for the primary reasoning model.
- **Hard-Prune Recovery**: Aggressive context management ensures the agent stays within 16k tokens, maintaining zero-latency responsiveness during massive refactors.

### 🌪️ The Sentinel Fleet & Sentient Telemetry
Tempest is protected by autonomous supervisors and real-time reasoning engine:
- **Reasoning Trace (Chain of Thought)**: View the agent's internal monologue in a dedicated side-pane.
- **Context Runway**: Predictive token management that prevents context overflow before it happens.
- **Thermal Guard**: Monitors hardware temperatures and autonomously triggers throttling or cooling pauses to protect your workstation.

---

## ⚡ The High-Fidelity TUI Experience
Tempest AI features a professional, industrial Terminal User Interface designed for high-stakes engineering.

![Tempest AI High-Fidelity TUI Interface](docs/tui_hero.png)

### 🛠️ Visual Dashboard Features:
- **Industrial Branding**: Centered high-fidelity ASCII logo with 🌪️ AI Core accents.
- **Real-Time Telemetry**: Live tracking of **CPU, RAM, GPU, and Thermals** in the sidebar.
- **Synchronized Reasoning**: Dynamic ASCII spinners that activate when the agent is "Thinking."
- **Fluid Token Streaming**: Messages stream into the chat area token-by-token for a high-speed experience.

---

## 🛠️ The Tempest Toolbox (50+ Native Tools)

### 📂 File & Workflow Suite
- **Navigation**: `project_atlas`, `tree`, `list_dir`, `search_files`.
- **Manipulation**: `read_file`, `write_file`, `patch_file`, `append_file`, `find_replace`, `create_directory`, `delete_file`.

### 💻 Development Suite
- **Build & Test**: `build_project`, `run_tests`, `run_command`.
- **Version Control**: `git_status`, `git_diff`, `git_commit`, `git_action`.
- **Process Control**: `run_background`, `read_process_logs`, `kill_process`, `watch_directory`.

### 🧠 Knowledge & RAG Suite
- **Semantic RAG**: `index_file_semantically`, `semantic_search`.
- **Memory**: `store_memory`, `recall_memory`.
- **Knowledge Base**: `distill_knowledge`, `recall_brain`.

---

## 🚀 Quick Start (One-Liner)

> [!IMPORTANT]
> **Requirements**: Rust **1.95+** is required. Ollama must be running locally.

```bash
git clone https://github.com/7empest462/tempest_ai.git && cd tempest_ai && cargo build --release && sudo cp target/release/tempest_ai /usr/local/bin/tempest_ai
```

### ❄️ Nix Development (Linux/macOS)
```bash
nix develop  # Pins Rust 1.95 + all native libraries
```

---

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
