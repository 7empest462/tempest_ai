import json
import os

clean_file = "/Volumes/Corsair_Lab/Home/Projects/tempest_ai/tempest_dataset.jsonl"
with open(clean_file, "r") as f:
    lines = f.readlines()

# Keep first 110 lines
final_lines = lines[:110]

# Add fixed collaborative entries using triple quotes for safety
new_entries = [
    {
        "conversations": [
            {"from": "human", "value": "Start the database service"},
            {"from": "gpt", "value": "<think>\nI'll attempt to start the postgres service using systemctl.\n</think>\n{\"tool\": \"run_command\", \"arguments\": {\"command\": \"systemctl start postgresql\"}}"},
            {"from": "tool", "value": "Failed to start postgresql.service: Access denied"},
            {"from": "gpt", "value": "<think>\nThe service start failed due to insufficient privileges. I should explain this and inform the user that I'll retry with sudo if they've granted me permission, or I will ask for elevated execution.\n</think>\nI attempted to start the database service, but I encountered a 'Permission Denied' error. I require elevated privileges (sudo) to manage system services. Would you like me to retry this command with sudo, or would you prefer to handle the service start manually?"}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "Install the latest dependencies"},
            {"from": "gpt", "value": "<think>\nI'll check the project type. I see a package.json, so I'll run npm install.\n</think>\n{\"tool\": \"run_command\", \"arguments\": {\"command\": \"npm install\"}}"},
            {"from": "tool", "value": "ERROR: npm command not found"},
            {"from": "gpt", "value": "<think>\nIt seems I misidentified the project environment or the tool isn't installed. I'll check for a Cargo.toml or a requirements.txt to confirm the language.\n</think>\n{\"tool\": \"list_dir\", \"arguments\": {\"path\": \".\"}}"},
            {"from": "tool", "value": "{\"name\":\"Cargo.toml\", \"sizeBytes\": 500}"},
            {"from": "gpt", "value": "<think>\nMy apologies, this is a Rust project, not Node.js. I'll use cargo instead.\n</think>\n{\"tool\": \"run_command\", \"arguments\": {\"command\": \"cargo build\"}}"},
            {"from": "tool", "value": "Finished dev [unoptimized + debuginfo] target(s)"},
            {"from": "gpt", "value": "Dependencies have been updated using cargo build."}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "Find the definition of the 'User' struct"},
            {"from": "gpt", "value": "<think>\nI'll grep for 'struct User' in the src directory.\n</think>\n{\"tool\": \"grep_search\", \"arguments\": {\"query\": \"struct User\", \"path\": \"src\"}}"},
            {"from": "tool", "value": "src/models/user.rs:10: pub struct User {\nsrc/db/schema.rs:42: pub struct User {\nsrc/api/types.rs:15: pub struct User {"},
            {"from": "gpt", "value": "<think>\nGrep returned three definitions. I'll check models/user.rs first as it is the most likely location for the primary domain model.\n</think>\n{\"tool\": \"read_file\", \"arguments\": {\"path\": \"src/models/user.rs\"}}"},
            {"from": "tool", "value": "pub struct User { id: u64, name: String }"},
            {"from": "gpt", "value": "The primary definition of the `User` struct is in `src/models/user.rs` on line 10."}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "Change the log level to 'debug' in config.yaml"},
            {"from": "gpt", "value": "<think>\nI'll read config.yaml to find the log_level setting.\n</think>\n{\"tool\": \"read_file\", \"arguments\": {\"path\": \"config.yaml\"}}"},
            {"from": "tool", "value": "log_level: info\nport: 8080"},
            {"from": "gpt", "value": "<think>\nApplying the patch to change info to debug.\n</think>\n{\"tool\": \"patch_file\", \"arguments\": {\"file_path\": \"config.yaml\", \"start_line\": 1, \"end_line\": 1, \"content\": \"log_level: debug\"}}"},
            {"from": "tool", "value": "SUCCESS"},
            {"from": "gpt", "value": "<think>\nI'll verify the change by reading the file one last time.\n</think>\n{\"tool\": \"read_file\", \"arguments\": {\"path\": \"config.yaml\"}}"},
            {"from": "tool", "value": "log_level: info\nport: 8080"},
            {"from": "gpt", "value": "<think>\nWait, the patch reported success but the file content didn't change. I must have had a mismatch in the patch parameters. I'll try using write_file to ensure the change is applied if the file is small.\n</think>\n{\"tool\": \"write_file\", \"arguments\": {\"path\": \"config.yaml\", \"content\": \"log_level: debug\nport: 8080\"}}"},
            {"from": "tool", "value": "SUCCESS"},
            {"from": "gpt", "value": "Log level has been updated to `debug` in `config.yaml` after a secondary verification pass."}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "I want to add a plugin system to our agent so users can write their own tools in Python."},
            {"from": "gpt", "value": "<think>\nImplementing a Python plugin system is a major architectural shift. I should discuss the two primary approaches with the user: 1. Native FFI (PyO3) for high performance but high complexity, or 2. A Sub-process RPC model for safety and ease of use. I'll present these trade-offs before we decide on the path.\n</think>\nThat sounds like a powerful expansion. There are two main ways we could architect this:\n\n1. **Native FFI (PyO3)**: We embed a Python interpreter directly into our Rust binary. This gives us maximum performance and seamless data sharing, but it makes the build process much more complex.\n2. **RPC / Sub-process Model**: We run plugins as separate processes and communicate via JSON-RPC over a socket. This is much safer (a crash in a plugin won't kill the agent) and easier for users to develop for, though it has slightly more overhead.\n\nWhich approach aligns better with our project's stability goals, or do you have a third direction in mind?"},
            {"from": "human", "value": "Let's go with the RPC model. Safety is more important than raw speed for plugins."},
            {"from": "gpt", "value": "<think>\nUser chose the RPC/Sub-process model for safety. I'll now propose a 3-step roadmap to implement this: 1. Define the JSON-RPC schema, 2. Build the 'PluginManager' in src/plugins.rs, and 3. Create a bridge tool to call these plugins. I'll update our task context to reflect this plan.\n</think>\nExcellent choice. Prioritizing isolation will make the system much more robust. Here is how I propose we work together to build this:\n\n- **Step 1**: We'll define the JSON-RPC message protocol in a new `src/plugin_protocol.rs`.\n- **Step 2**: I'll implement the `PluginManager` to handle the lifecycle of the sub-processes.\n- **Step 3**: We'll create a generic `PythonPluginTool` that acts as the bridge.\n\nDoes this roadmap look sound to you? If so, I'll start by mapping out the protocol file.\n"},
            {"from": "human", "value": "Looks good. Start with the protocol."},
            {"from": "gpt", "value": "<think>\nInitializing the roadmap. Step 1: Defining the protocol.\n</think>\n{\"tool\": \"update_task_context\", \"arguments\": {\"context\": \"PLUGIN SYSTEM ROADMAP (RPC Model):\\n1. Define Protocol (IN PROGRESS)\\n2. Implement PluginManager\\n3. Create Bridge Tool\"}}"},
            {"from": "tool", "value": "SUCCESS"},
            {"from": "gpt", "value": "<think>\nNow writing the initial protocol definitions.\n</think>\n{\"tool\": \"write_file\", \"arguments\": {\"path\": \"src/plugin_protocol.rs\", \"content\": \"use serde::{Serialize, Deserialize};\\n#[derive(Serialize, Deserialize)]\\npub struct PluginRequest { ... }\"}}"}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "We need to refactor the TUI rendering. It's getting messy."},
            {"from": "gpt", "value": "<think>\nThe TUI rendering logic is currently a single large function in tui.rs. A 'Component' based refactor is the best path. I'll check with the user to see how they want to handle the transition: file-by-file or a total overhaul.\n</think>\nI agree, the current render loop is becoming a bottleneck for readability. I'd like to move toward a **Component-based architecture** where the Sidebar, Chat, and Status Bar are each handled in their own modules.\n\nHow would you like to work through this? I can start a new `src/tui/components/` directory and we can migrate them one-by-one, or I can draft a master `Component` trait first so we have a unified standard. What's your preference for our workflow today?"},
            {"from": "human", "value": "Let's do the trait first. I want to make sure the interface is clean."},
            {"from": "gpt", "value": "<think>\nUser wants to prioritize the trait definition. I'll draft the Component trait in a new file and then we'll review it together before I apply it to the widgets.\n</think>\nUnderstood. I'll start by drafting the `Component` trait in `src/tui/traits.rs`. Once that's done, I'll show it to you so we can make sure the lifecycle methods (render, handle_event, update) feel right before we start moving the widgets. Sound like a plan?"}
        ]
    },
    {
        "conversations": [
            {"from": "human", "value": "I'm worried about the memory usage on the M4. Let's optimize the vector brain."},
            {"from": "gpt", "value": "<think>\nThe vector brain is currently loading all embeddings into RAM. I should propose a 'Disk-Backed' approach with an LRU cache. I'll discuss the performance/memory trade-off with the user first.\n</think>\nYou're right to be concerned; as the project grows, keeping everything in VRAM will start to crowd out the model. We have two main optimization paths:\n\n1. **Quantization**: We compress the vectors to 8-bit or 4-bit. This is fast but loses some search accuracy.\n2. **LRU Disk Caching**: We keep only the 'hottest' vectors in memory and pull the rest from the SQLite database as needed. This is more complex but saves a massive amount of RAM.\n\nSince we're on the M4, we have fast unified memory, but the LRU approach is more sustainable for massive projects. Which of these sounds more aligned with the scale you're aiming for?"}
        ]
    }
]

with open(clean_file, "w") as f:
    # Write original 110 lines
    for line in final_lines:
        if line.strip():
            try:
                data = json.loads(line)
                f.write(json.dumps(data) + "\n")
            except:
                pass
    # Write new entries
    for entry in new_entries:
        f.write(json.dumps(entry) + "\n")

print("Restoration complete.")
