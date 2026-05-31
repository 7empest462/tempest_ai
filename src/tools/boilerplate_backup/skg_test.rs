use skg_tool::{ToolError, ToolCallContext, ToolDyn};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

#[derive(serde::Deserialize, schemars::JsonSchema, serde::Serialize)]
pub struct EchoArgs {
    pub message: String,
}

pub struct EchoTool;

impl ToolDyn for EchoTool {
    fn name(&self) -> &str {
        "skg_echo"
    }

    fn description(&self) -> &str {
        "Echoes back the message provided. Use this to test Skelegent integration."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back."
                }
            },
            "required": ["message"]
        })
    }

    fn call(
        &self,
        input: Value,
        _ctx: &ToolCallContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let args: EchoArgs = serde_json::from_value(input)
                .map_err(|e| ToolError::ExecutionFailed(format!("Invalid arguments: {}", e)))?;
            
            Ok(serde_json::json!({
                "echo": args.message,
                "status": "success"
            }))
        })
    }
}

