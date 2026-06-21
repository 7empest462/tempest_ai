// ==========================================
// 🌪️ SKELEGENT NATIVE TOOL IMPLEMENTATIONS
// ==========================================
// These are the modern #[skg_tool] versions of the Tempest toolset.
// They register under the SAME names as their legacy counterparts
// so the LLM's tool routing behavior is preserved.
//
// Controlled by `tool_engine = "skg"` in config.toml.
// Default: "legacy" (the original AgentTool implementations).

pub mod agent_ops;
pub mod ast;
pub mod atlas;
pub mod csv;
pub mod database;
pub mod demo;
pub mod developer;
pub mod echo;
pub mod editing;
pub mod execution;
pub mod file;
pub mod git;
pub mod knowledge;
pub mod memory;
pub mod network;
pub mod network_manager;
pub mod privilege;
pub mod process;
pub mod rust;
pub mod search;
pub mod service_manager;
pub mod system;
pub mod telemetry;
pub mod terminal;
pub mod threat_scanner;
pub mod utilities;
pub mod visualization;
pub mod wasm_sandbox;
pub mod web;
