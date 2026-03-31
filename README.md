# 🌪️ Tempest AI (Project Smart-Brain)
**The Hardware-Aware, RAG-Powered Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat aliases, Tempest is a **Stateful Intelligence** that monitors your hardware, manages a persistent conceptual brain, and operates with a disciplined Planning/Execution lifecycle.

---

## 🚀 "Smart-Brain" Capabilities

### 🌡️ 1. Hardware-Aware Sentience
Tempest is the first local agent that is "Sentient" of its host. It injects real-time **CPU, GPU, RAM, and Thermal telemetry** into its reasoning loop, allowing it to pace complex builds and multi-threaded tasks based on your machine's actual load.

### 🛡️ 2. Disciplined Guardrails
- **Planning vs. Execution**: Tempest starts every session in **PLANNING MODE**. It researches your codebase and presents a strategy. It CANNOT modify your system until you explicitly approve the plan.
- **Loop Interception**: Implements a "Sentinel" layer that detects duplicate tool calls and hallucinations, forcing the agent to pivot or ask for help before wasting tokens.

### 🧠 3. Reflective Memory (The Sketchpad)
Tempest maintains a persistent `{task_context}` known as the **Sketchpad**. This ensures mission continuity across long-running tasks, preventing the "forgetting" common in long LLM conversations.

### 📂 4. Deep Semantic Search (Vector RAG)
Powered by `nomic-embed-text`, Tempest features a local **Vector Brain**.
- **`index_file_semantically`**: Tempest "digests" your code into mathematical concepts.
- **`semantic_search`**: Search your codebase by *meaning* rather than keywords. ("Find where we handle the telemetry overflow.")

### 🕵️ 5. Sub-Agent Delegation
For complex research missions, Tempest can spawn specialized **Sub-Agents**. These assistants perform deep-dives on documentation or logs in parallel, returning a distilled "Mission Report" to the Principal Agent.

### 🧹 6. Autonomous Verification (Clippy)
Every file modification triggers an automatic **Self-Correction Hook**. Tempest lint-checks and verifies its own work immediately after writing, catching syntax errors before you even see them.

---

## 🛠️ Setup & Configuration

### Prerequisites
1. **Ollama**: [Download Ollama](https://ollama.com)
2. **Models**:
   ```bash
   ollama pull qwen2.5-coder:7b      # Recommended Principal Model
   ollama pull nomic-embed-text      # Required for Semantic Brain
   ollama pull phi3:latest           # Recommended Sub-Agent Model
   ```

### Configuration (`config.toml`)
Tempest looks for a config file at `~/.config/tempest_ai/config.toml`.
```toml
model = "qwen2.5-coder:7b"
sub_agent_model = "phi3:latest"
history_path = "~/.tempest_history"
encrypt_history = true
```

---

## 📦 Installation

1. **Build the Binary**:
   ```bash
   cargo build --release
   ```
2. **Install Globally**:
   ```bash
   sudo cp target/release/tempest_ai /usr/local/bin/tempest_ai
   ```

---

## 💻 Usage Lifecycle

1. **RESEARCH**: Tempest analyzes the environment in Planning Mode.
2. **PLAN**: It presents a numbered list of actions.
3. **APPROVE**: You say "Go ahead" or "Proceed."
4. **EXECUTE**: Tempest toggles out of planning mode and begins the work.
5. **VERIFY**: Tempest run-checks its output and summarizes the mission.

---

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
