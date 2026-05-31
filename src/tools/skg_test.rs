use skg_tool_macro::skg_tool;
use skg_tool::{ToolError, ToolCallContext};

#[skg_tool(
    name = "skg_echo",
    description = "Echoes back the message provided. Use this to test Skelegent integration."
)]
pub async fn skg_echo(
    message: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    Ok(serde_json::json!({
        "echo": message,
        "status": "success"
    }))
}
