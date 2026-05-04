# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Primary development commands
### Rust backend (root crate)
- Build (debug): `cargo build`
- Build (release): `cargo build --release`
- Run default TUI mode: `cargo run --release`
- Run CLI mode (non-TUI): `cargo run --release -- --cli`
- Run with MLX backend (Apple Silicon): `cargo run --release -- --mlx`
- Run as MCP server (JSON-RPC over stdio): `cargo run --release -- --mcp_server`
- Run tests: `cargo test`
- Run a single test (exact): `cargo test checkpoint::tests::test_checkpoint_and_undo -- --exact`
- Run benchmarks: `cargo bench`
- Lint: `cargo clippy --all-targets --all-features`
- Format: `cargo fmt --all`

### VS Code extension (`vscode-tempest/`)
- Install deps: `npm install`
- Compile extension: `npm run compile`
- Watch mode: `npm run watch`

## Configuration and runtime notes
- Main config lookup order is implemented in `src/main.rs`:
  1. `--config` path if provided
  2. `./config.toml` (repo-local override)
  3. OS config locations (for example `~/.config/tempest_ai/config.toml`)
- Repository-local `config.toml` is actively used for model routing and sampling defaults.
- Metrics exporter starts at process boot and binds to `0.0.0.0:7777`.

## High-level architecture
### 1) Entry and mode selection
- `src/main.rs` is the executable entrypoint and orchestrates:
  - CLI parsing (`--cli`, `--mlx`, `--bridge`, `--mcp_server`, daemon modes)
  - Config loading and model selection
  - `Agent::new(...)` construction
  - Runtime path selection: TUI loop, plain CLI loop, or headless MCP server.

### 2) Agent orchestration state machine
- `src/agent.rs` contains the core orchestration logic:
  - `AgentStreamState` models turn lifecycle (`Thinking`, `PendingTools`, `ExecutingTools`, `StreamingContent`, `Done`).
  - Planner/executor phase switching is explicit via `AgentPhase` and model routing.
  - `run(...)` drives iterative transitions until completion/interrupt.
- `initialize_session(...)` composes system prompt + tool schema + active rule injections before each user turn.

### 3) Inference backends and streaming
- `src/inference.rs` abstracts inference behind `Backend`:
  - Ollama backend
  - MLX backend on macOS (with optional paged attention)
  - AI bridge backend for provider abstraction
- `stream_chat(...)` is the token/tool-call streaming boundary used by the agent loop.

### 4) Tooling architecture
- Tool trait and shared context live in `src/tools/mod.rs` (`AgentTool`, `ToolContext`).
- `Agent::new` registers a large native tool set and then builds a smaller `tool_registry` subset for prompt-size control.
- Execution path:
  - Tool calls are normalized/repaired, deduplicated, then dispatched via `executor_dispatch(...)`.
  - Modifying tools are checkpointed and can be reverted (`/undo`).

### 5) Safety and context control layers
- `src/sentinel.rs` applies runtime guards (loop/repetition detection, context pressure, privilege warnings, hallucination checks, thermal/build checks).
- `src/context_manager.rs` tracks token pressure and performs history compaction through a sub-model when context is near saturation.
- `src/checkpoint.rs` provides reversible file-change batches with diff preview support and undo stack behavior.

### 6) TUI event-driven UI
- `src/tui.rs` implements the terminal dashboard and event bus (`AgentEvent`).
- Main loop receives streamed tokens/status updates and supports command palette, explorer, and telemetry HUD.

### 7) MCP integration model
- External MCP clients are implemented in `src/mcp.rs`.
- `Agent::initialize_mcp(...)` connects to configured servers, discovers tools dynamically, and registers proxied MCP tools into the same tool registry path as native tools.

## Additional repository context relevant to agents
- `README.md` is the authoritative user-facing quick-start and feature overview.
- `MANUAL.md` documents slash commands and runtime behavior details used by TUI/CLI flows.
- No repository-level CLAUDE/Cursor/Copilot instruction files were found (`CLAUDE.md`, `.cursorrules`, `.cursor/rules/`, `.github/copilot-instructions.md`).
