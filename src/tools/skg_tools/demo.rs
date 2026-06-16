use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

#[skg_tool(
    name = "demo_tool",
    description = "A demonstration tool using skg-tool-macro to showcase clean, declarative tool creation."
)]
pub async fn demo(
    message: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    Ok(serde_json::json!({
        "received_message": message,
        "status": "hello from skg-tool-macro!"
    }))
}
