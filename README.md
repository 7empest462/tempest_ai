# 🌪️ Tempest AI (Project Smart-Brain)
**The Hardware-Aware, Native-Schema Autonomous Engineer.**

Tempest AI is a high-performance, Rust-based autonomous agent designed to be your local "Principal Engineer." Unlike standard chat aliases, Tempest is a **Stateful Intelligence** that monitors your hardware, manages a persistent conceptual brain via native tool-calling schemas, and operates with a disciplined Planning/Execution lifecycle.

---

## 🚀 "Native-Engine" Capabilities

### ⚡ 1. Native Tool-Calling Architecture
Tempest has been migrated to the **`ollama-rs` 0.3.4** typed tool-calling framework. 
- **Strongly Typed**: Every tool is defined using `schemars` JSON schemas, eliminating brittle regex-based Markdown parsing.
- **Improved Autonomy**: The LLM receives exact structural requirements for every function, leading to a 90% reduction in malformed tool calls.
- **Multi-Turn Chaining**: Supports multiple sequential tool executions in a single reasoning step.

### 🧠 2. Categorized Long-Term Memory
Tempest features a persistent SQLite-backed **Conceptual Brain** with `#tagging` support.
- **Contextual Retrieval**: Store facts with searchable tags (e.g., `#config`, `#todo`, `#db`).
- **Fuzzy Recall**: Retrieve memories via topic names or associated tags, ensuring the agent "remembers" the right context at the right time.

### 🌡️ 3. Hardware-Aware Sentience
Tempest is the first local agent that is truly "Sentient" of its host. It injects real-time **CPU, GPU, RAM, and Thermal telemetry** into its reasoning loop.
- **Load-Adaptive**: The agent can autonomously slow down or pivot tasks if it detects system memory is critically low or thermals are spiking.
- **Cross-Platform**: Full telemetry support for macOS (Apple Silicon) and Linux (sysfs/hwmon).

### 🛡️ 4. Disciplined Guardrails (The Master Switch)
- **Planning Mode**: Tempest starts every session in a locked state. It can research and plan, but MUST physically call the **`toggle_planning`** tool to unlock system-modifying actions.
- **Loop Interception**: A "Sentinel" layer detects duplicate tool sequences and "hallucinated loops," forcing the agent to self-correct before wasting tokens.

### 🕵️ 5. Sub-Agent Delegation
For complex research or parallel debugging, Tempest can spawn specialized **Sub-Agents**. These assistants focus on localized tasks and return a distilled report to the Principal Agent.

---

## 🚀 Quick Start (One-Liner)

```bash
git clone https://github.com/7empest462/tempest_ai.git && cd tempest_ai && cargo build --release && sudo cp target/release/tempest_ai /usr/local/bin/tempest_ai
```

---

## 🛠️ Setup & Configuration

### 💎 Prerequisites
1. **Ollama**: [Download Ollama](https://ollama.com)
2. **Models**:
   ```bash
   ollama pull qwen2.5-coder:7b      # Principal Model (Recommended for Python/Rust)
   ollama pull mistral:7b           # Alternative: Great for general reasoning
   ollama pull nomic-embed-text      # REQUIRED: For the Vector Brain (Semantic RAG)
   ollama pull phi3:latest           # Recommended: For Sub-Agent tasks
   ```

### ⚙️ Configuration
Tempest looks for its config at `~/.config/tempest_ai/config.toml`.
```toml
model = "qwen2.5-coder:7b"
sub_agent_model = "phi3:latest"
history_path = "~/.tempest_history"
encrypt_history = true
```

---

## 💻 Usage Lifecycle

1. **RESEARCH**: Tempest orientates using `project_atlas` and `tree` in Planning Mode.
2. **PLAN**: It presents a detailed architectural strategy.
3. **TOGGLE**: Once approved, Tempest calls `toggle_planning` to enter **EXECUTION MODE**.
4. **EXECUTE**: The agent performs file edits, git operations, and system commands.
5. **VERIFY**: Tempest run-checks its output (Self-Correction) and summarizes the mission outcome.

---

**Tempest AI is built for engineers who value privacy, speed, and autonomous reliability.** 🌬️🦾
