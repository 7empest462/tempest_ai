// ==========================================
// 🧪 SKG WASM SANDBOX TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use crate::wasm_engine::WasmSandboxEngine;

// ── wasm_safe_calc ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "wasm_safe_calc",
    description = "Executes a safe, resource-capped mathematical calculation inside an isolated, fuel-limited WASM sandbox container."
)]
pub async fn wasm_safe_calc(
    lh: i32,
    rh: i32,
    op: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let op_lower = op.to_lowercase();

    if op_lower != "add" && op_lower != "sub" && op_lower != "mul" && op_lower != "div" {
        return Err(ToolError::ExecutionFailed(format!(
            "Invalid operation '{}'. Supported: 'add', 'sub', 'mul', 'div'",
            op
        )));
    }

    if op_lower == "div" && rh == 0 {
        return Err(ToolError::ExecutionFailed(
            "WASM Execution Error: Division by zero is prohibited.".to_string()
        ));
    }

    let engine = WasmSandboxEngine::new()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create WASM engine: {}", e)))?;
    let res = engine.run_calculator(lh, rh, &op_lower)
        .map_err(|e| ToolError::ExecutionFailed(format!("WASM execution failed: {}", e)))?;

    Ok(serde_json::Value::String(format!(
        "SUCCESS: Sandboxed WASM operation '{}' ({} and {}) returned result: {}",
        op_lower, lh, rh, res
    )))
}
