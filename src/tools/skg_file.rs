use skg_tool_macro::skg_tool;
use skg_tool::{ToolError, ToolCallContext};
use std::fs;

#[skg_tool(
    name = "skg_read_file",
    description = "Reads the contents of a file. Use this to examine code or configuration."
)]
pub async fn skg_read_file(
    path: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    
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
    
    Ok(serde_json::Value::String(content))
}

#[skg_tool(
    name = "skg_write_file",
    description = "Writes content to a file. Warning: This overwrites existing content."
)]
pub async fn skg_write_file(
    path: String,
    content: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let content_owned = content;
    
    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        if let Some(parent) = std::path::Path::new(&path_owned).parent() {
            fs::create_dir_all(parent).map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directory: {}", e)))?;
        }
        
        fs::write(&path_owned, content_owned)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file {}: {}", path_owned, e)))?;
        
        Ok(())
    }).await.map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;
    
    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Successfully wrote to {}", path)
    }))
}
