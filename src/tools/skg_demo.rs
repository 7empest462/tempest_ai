use skg_tool_macro::skg_tool;
use skg_tool::{ToolError, ToolCallContext};

#[skg_tool(
    name = "skg_demo_tool",
    description = "A demonstration tool using skg-tool-macro to showcase clean, declarative tool creation."
)]
pub async fn skg_demo(
    message: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    Ok(serde_json::json!({
        "received_message": message,
        "status": "hello from skg-tool-macro!"
    }))
}
