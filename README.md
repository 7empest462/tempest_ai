# 🌪️ Tempest AI V2
**The fully autonomous, multi-threaded LLM engineer for your local terminal.**

Tempest AI is a blazing-fast, Rust-based autonomous coding agent powered entirely by local LLMs via Ollama. Once you give it a prompt, it enters an infinite context-protected loop where it natively searches your hard drive, scrapes the web for documentation, patches code surgically, and even spawns concurrent backend servers—all while preserving your privacy.

If it ever gets confused, it pauses its execution matrix and asks you for clarification. It's the open-source equivalent to Anthropic's multi-billion dollar Claude agent.

## 🚀 Features
- **Semantic Code Search**: Instant `.gitignore`-aware file querying powered by Mac Unix native `rg` (Ripgrep) binaries.
- **Surgical File Editing**: Allows the LLM to rewrite specific code blocks by targeting exact line numbers instead of randomly regenerating massive multi-thousand-line files.
- **Background Servers**: The agent can boot `npm run dev` or `cargo run` cleanly into a parallel thread and instantly query its output logs without crashing the active environment.
- **Rolling Memory Compression**: Natively slides your conversation history via a hidden `.HistorySummarizer()` to ensure you never max out your LLM token limit.
- **Interactive Prompts**: A mid-task LLM suspension architecture that automatically requests STDIN feedback from the human if ambiguity blocks an execution.
- **Dynamic Guardrails**: Instantly intercepts hallucinated terminal outputs and forces the LLM to recursively structure its thoughts inside secure JSON schemas.

---

## 🛠️ Prerequisites
Before running Tempest AI, make sure you have the following installed on your machine:

1. **Rust & Cargo**: [Install Rust](https://www.rust-lang.org/tools/install)
2. **Ollama**: [Install Ollama](https://ollama.ai/download) to run models locally.
3. **Ripgrep** *(Optional but Highly Recommended)*: `brew install ripgrep` for blazing-fast codebase search.

---

## 📦 Installation

To test Tempest AI locally or install it entirely onto your global path:

1. **Clone the repository**
```bash
git clone https://github.com/yourusername/tempest_ai.git
cd tempest_ai
```

2. **Pull the Qwen Model**
Tempest AI defaults to utilizing the highly capable `qwen2.5-coder:7b` programming model.
```bash
ollama run qwen2.5-coder:7b
```

3. **Compile the Release Binary**
Compile the optimized version for maximum speed.
```bash
cargo build --release
```

4. **Install Globally (Optional)**
Move the binary into your PATH so you can summon it from any folder on your computer!
```bash
sudo cp target/release/tempest_ai /usr/local/bin/tempest_ai
```

---

## 💻 Usage
To start using Tempest AI, navigate to any empty directory or existing project folder and simply type:

```bash
tempest_ai
```

The system will verify the Ollama connection, validate your system's OS and architectures dynamically, and prompt you:
```text
>> 
```

**Just type what you want built!**
> *"Spin up a Next.js server locally, navigate to `app/page.tsx`, and convert the hero component into a dark-mode tailwind layout. Once it's built, ping the localhost router to ensure it renders correctly."*

The LLM will automatically launch tools, write files, inspect logs, and communicate directly with you until the job is done.
