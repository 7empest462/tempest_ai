use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

#[skg_tool(
    name = "echo",
    description = "Echoes back the message provided. Use this to test Skelegent integration."
)]
pub async fn echo(
    message: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    Ok(serde_json::json!({
        "echo": message,
        "status": "success"
    }))
}
