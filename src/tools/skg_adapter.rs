use std::sync::Arc;
use skg_tool::{ToolDyn, ToolCallContext, ToolError};
use serde_json::Value;
use std::pin::Pin;
use std::future::Future;
use crate::tools::AgentTool;

/// A wrapper that makes any AgentTool implement ToolDyn.
pub struct SkgToolAdapter {
    pub inner: Arc<dyn AgentTool>,
}

impl ToolDyn for SkgToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> Value {
        // ollama-rs ToolInfo contains the schema in function.parameters
        let info = self.inner.tool_info();
        serde_json::to_value(info.function.parameters).unwrap_or(Value::Object(Default::default()))
    }

    fn call(
        &self,
        input: Value,
        ctx: &ToolCallContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + '_>> {
        // Extract ToolContext from deps
        let context_res = ctx.deps::<Arc<crate::tools::ToolContext>>()
            .cloned()
            .ok_or_else(|| ToolError::ExecutionFailed("Missing Tempest ToolContext in deps".to_string()));
            
        let inner = self.inner.clone();
        
        Box::pin(async move {
            let context = context_res?;
            match inner.execute(&input, (*context).clone()).await {
                Ok(res) => Ok(Value::String(res)),
                Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
            }
        })
    }
    
    fn requires_approval(&self) -> bool {
        self.inner.is_modifying()
    }
}
