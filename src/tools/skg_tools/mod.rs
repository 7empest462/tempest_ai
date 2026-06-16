// ==========================================
// 🌪️ SKELEGENT NATIVE TOOL IMPLEMENTATIONS
// ==========================================
// These are the modern #[skg_tool] versions of the Tempest toolset.
// They register under the SAME names as their legacy counterparts
// so the LLM's tool routing behavior is preserved.
//
// Controlled by `tool_engine = "skg"` in config.toml.
// Default: "legacy" (the original AgentTool implementations).

pub mod demo;
pub mod echo;
pub mod file;
pub mod execution;
pub mod search;
pub mod git;
pub mod web;
pub mod memory;
pub mod editing;
pub mod agent_ops;
pub mod process;
pub mod terminal;
pub mod knowledge;
pub mod utilities;
pub mod ast;
pub mod rust;
pub mod wasm_sandbox;
pub mod threat_scanner;
pub mod csv;
pub mod telemetry;
pub mod network_manager;
pub mod service_manager;
pub mod developer;
pub mod database;
pub mod network;
pub mod atlas;
pub mod system;
pub mod privilege;
pub mod visualization;

