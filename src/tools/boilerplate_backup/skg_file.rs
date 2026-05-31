use skg_tool::{ToolError, ToolCallContext, ToolDyn};
use serde_json::Value;
use std::fs;
use std::future::Future;
use std::pin::Pin;

#[derive(serde::Deserialize, schemars::JsonSchema, serde::Serialize)]
pub struct ReadFileArgs {
    /// Path to the file to read.
    pub path: String,
}

pub struct ReadFileTool;

impl ToolDyn for ReadFileTool {
    fn name(&self) -> &str {
        "skg_read_file"
    }

    fn description(&self) -> &str {
        "Reads the contents of a file. Use this to examine code or configuration."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read."
                }
            },
            "required": ["path"]
        })
    }

    fn call(
        &self,
        input: Value,
        _ctx: &ToolCallContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let args: ReadFileArgs = serde_json::from_value(input)
                .map_err(|e| ToolError::ExecutionFailed(format!("Invalid arguments: {}", e)))?;
            
            let path_owned = shellexpand::tilde(&args.path).to_string();
            
            let content = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
                let metadata = fs::metadata(&path_owned)
                    .map_err(|e| ToolError::ExecutionFailed(format!("File not found or inaccessible: {}", e)))?;
                
                if metadata.len() > 1_000_000 {
                    return Err(ToolError::ExecutionFailed(format!("File too large: {} bytes (max 1MB)", metadata.len())));
                }
                
                let bytes = fs::read(&path_owned)
                    .map_err(|e| ToolError::ExecutionFailed(format!("IO error reading {}: {}", path_owned, e)))?;
                let content = match String::from_utf8(bytes) {
                    Ok(s) => s,
                    Err(_) => return Err(ToolError::ExecutionFailed(format!("BINARY FILE DETECTED: '{}' cannot be read as text.", path_owned))),
                };
                
                Ok(content)
            }).await.map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;
            
            Ok(Value::String(content))
        })
    }
}


#[derive(serde::Deserialize, schemars::JsonSchema, serde::Serialize)]
pub struct WriteFileArgs {
    /// Path to the file to write.
    pub path: String,
    /// Content to write to the file.
    pub content: String,
}

pub struct WriteFileTool;

impl ToolDyn for WriteFileTool {
    fn name(&self) -> &str {
        "skg_write_file"
    }

    fn description(&self) -> &str {
        "Writes content to a file. Warning: This overwrites existing content."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn call(
        &self,
        input: Value,
        _ctx: &ToolCallContext,
    ) -> Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let args: WriteFileArgs = serde_json::from_value(input)
                .map_err(|e| ToolError::ExecutionFailed(format!("Invalid arguments: {}", e)))?;
            
            let path_owned = shellexpand::tilde(&args.path).to_string();
            let content_owned = args.content;
            
            tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
                // Ensure parent directory exists
                if let Some(parent) = std::path::Path::new(&path_owned).parent() {
                    fs::create_dir_all(parent).map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directory: {}", e)))?;
                }
                
                fs::write(&path_owned, content_owned)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file {}: {}", path_owned, e)))?;
                
                Ok(())
            }).await.map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;
            
            Ok(serde_json::json!({
                "status": "success",
                "message": format!("Successfully wrote to {}", args.path)
            }))
        })
    }
}
