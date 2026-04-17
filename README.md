# 🌪️ Tempest AI (Project Smart-Brain)
**The Hardware-Aware, Native-Schema Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat aliases, Tempest is a **Stateful Intelligence** that monitors your hardware, manages a persistent conceptual brain via native tool-calling schemas, and operates with a disciplined Planning/Execution lifecycle.

---

## 🚀 "Native-Engine" Capabilities

### ⚡ 1. Native Tool-Calling Architecture
Tempest is powered by the **`ollama-rs` 0.3.4** typed tool-calling framework. 
- **Strongly Typed**: Every tool is defined using `schemars` JSON schemas, eliminating brittle regex-based Markdown parsing.
- **Schema Emancipation**: The agent is hardened against hallucinated network reconnaissance and is optimized to route directly to native tools like `get_stock_price`.
- **Improved Autonomy**: The LLM receives exact structural requirements for every function, leading to a 90% reduction in malformed tool calls.
- **Multi-Turn Chaining**: Supports multiple sequential tool executions in a single reasoning step.

### 🧠 2. Categorized Long-Term Memory
Tempest features a persistent SQLite-backed **Conceptual Brain** with `#tagging` support.
- **Contextual Retrieval**: Store facts with searchable tags (e.g., `#config`, `#todo`, `#db`).
- **Fuzzy Recall**: Retrieve memories via topic names or associated tags, ensuring the agent "remembers" the right context at the right time.

### 🛡️ 3. Sentinel Fleet (Autonomous Supervision)
Tempest is the first local agent protected by an autonomous **Sentinel Fleet**. Rather than waiting for errors to occur, a suite of deterministic supervisors monitors every reasoning turn:
- **Context Runway**: Forecasts context usage and triggers surgical compaction before the LLM overflows.
- **Privilege Escalator**: Detects protected resource access and manages the secure escalation protocol.
- **Compiler Guard**: Identifies "whack-a-mole" debugging patterns and forces strategic pivots during broken builds.
- **Build Watcher**: Prevents "Stale Testing" by ensuring the agent is always aware of Bin-to-Source synchronization.
- **Thermal Guard**: Monitors hardware temperatures and autonomously triggers thermal throttling or cooling pauses to project your physical workstation.

### 🛑 4. Real-Time Agent Interjection
The reasoning loop is no longer a "black box."
- **Esc Interrupt**: Pressing **`Esc`** immediately halts the current thought process or model stream, returning control to the user instantly.
- **Safety Keybindings**: `Ctrl+C` for application termination, `Esc` for surgical agent halts.

### 🔍 5. Hardened Conceptual Brain
The RAG (Retrieval Augmented Generation) engine has been hardened for production-industrial use.
- **Deduplication**: Automatic flush-and-rebuild for semantic indexing ensures no redundant memory copies.
- **Ghost Purging**: Periodic cleanup of "ghost" entries (references to deleted or moved files), keeping the agent grounded in the actual workspace state.
- **Improved Retrieval**: 70% reduction in "hallucinated file awareness" through surgical index sanitization.

#### Profiling Commands
```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo flamegraph --bench tool_performance

# Runtime introspection with tokio-console
RUSTFLAGS="--cfg tokio_unstable" cargo run --features tokio-console

# Then connect with: tokio-console
```

### 🛡️ 6. "Principal Engineer" Privilege Protocol
Tempest features a secure, user-approved protocol for session-based privilege escalation.
- **Secure Sudo-Bridge**: The agent uses the `request_privileges` tool to proactively ask for root access via a high-fidelity TUI prompt.
- **Non-blocking Elevation**: Commands are wrapped in `sudo -n` (non-interactive mode), ensuring the agent never hangs waiting for a ghost password challenge.
- **Transparent Logic**: Users confirm escalation rationale before the agent gains root-level sensor and actuator access.

### ⚡ 7. High-Performance Async Hardening
The agent core has been hardened for zero-latency autonomous development.
- **Non-Blocking I/O**: All blocking filesystem operations (`read_file`, `write_file`, `patch_file`, `append_file`, `find_replace`) are offloaded to dedicated `spawn_blocking` thread pools.
- **Responsive Reasoning**: Ensuring the reasoning engine remains 100% responsive even during massive repository refactors or heavy system I/O.

---

## ⚡ The High-Fidelity TUI Experience

Tempest AI features a professional, industrial Terminal User Interface designed for high-stakes engineering. This "Principal Engineer" dashboard provides real-time situational awareness and fluid interaction.

![Tempest AI High-Fidelity TUI Interface](docs/tui_hero.png)

### 🛠️ Visual Dashboard Features:
- **Industrial Branding**: Centered high-fidelity ASCII logo with 🌪️ AI Core accents.
- **Real-Time Telemetry**: Live tracking of **CPU, RAM, GPU, and Thermals** directly in the sidebar.
- **Synchronized Reasoning**: Dynamic ASCII spinners that activate when the agent is in a "Thinking" state.
- **Secure Escalation Prompts**: Dedicated UI blocks for user confirmation of privilege requests.
- **Fluid Token Streaming**: Messages stream into the chat area token-by-token for a responsive, high-speed experience.

---

## 🛠️ The Tempest Toolbox (50+ Native Tools)

Tempest comes equipped with a vast array of specialized sensors and actuators, organized into functional suites:

### 📂 File & Workflow Suite
- **Navigation**: `project_atlas`, `tree`, `list_dir`, `search_files`.
- **Manipulation**: `read_file`, `write_file`, `patch_file`, `append_file`, `find_replace`.
- **Logic**: `diff_files`, `edit_file_with_diff`.

### 💻 Development Suite
- **Build & Test**: `build_project`, `run_tests`, `run_command`.
- **Version Control**: `git_status`, `git_diff`, `git_commit`, `git_action`.
- **Process Control**: `run_background`, `read_process_logs`, `kill_process`, `watch_directory`.

### 🧠 Knowledge & RAG Suite
- **Semantic RAG**: `index_file_semantically`, `semantic_search`.
- **Memory**: `store_memory`, `recall_memory` (with tagging).
- **Knowledge Base**: `distill_knowledge`, `recall_brain`.

### 🛰️ System & Telemetry Suite
- **Hardware**: `system_info`, `gpu_diagnostics`, `system_telemetry`, `system_diagnostic_scan`.
- **Deep Linux**: `kernel_diagnostic`, `systemd_manager`, `linux_process_analyzer`.

### 🌐 Network & Web Suite
- **Connectivity**: `low_level_icmp_diagnostic`, `network_sniffer`.
- **Web**: `search_web`, `read_url`, `raw_http_fetch`, `download_file`.
- **Finance**: `get_stock_price`.

### ⚙️ Agent Ops & Utilities
- **Meta**: `spawn_sub_agent`, `toggle_planning`, `request_privileges`, `update_task_context`, `ask_user`.
- **Utils**: `clipboard`, `notify`, `env_var`, `chmod`.

---

### 🛡️ Guardrails & Safety
- **Planning Mode**: Tempest starts every session in a locked state. It can research and plan, but MUST physically call the **`toggle_planning`** tool to unlock system-modifying actions.
- **Turn Watchdog**: A "Sentinel" layer detects when the agent tries to finish turn silently after a tool call. It automatically intercedes with a reprimand to force a final summary.
- **Tool Repair Layer**: An internal fuzzy-mapper that heals minor tool hallucinations (e.g., `stock_price` -> `get_stock_price`) and arg-name errors on the fly.

---

## 🚀 Quick Start (One-Liner)

> [!IMPORTANT]
> **Requirements**: Rust **1.95+** is required to support the modern async features and tool-calling schemas.

```bash
git clone https://github.com/7empest462/tempest_ai.git && cd tempest_ai && cargo build --release && sudo cp target/release/tempest_ai /usr/local/bin/tempest_ai
```

### ❄️ Nix Development (Linux/macOS)
If you have Nix installed, you can skip manual toolchain setup:
```bash
# Enter the modern flake shell (pins Rust 1.95 + all native libraries)
nix develop

# Legacy nix shell fallback
nix-shell
```

## ⚙️ Configuration
Tempest looks for its config at `~/.config/tempest_ai/config.toml`.
```toml
model = "qwen2.5-coder:7b"
sub_agent_model = "qwen2.5-coder:3b"
history_path = "~/.tempest_history"
encrypt_history = true
```

---

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
