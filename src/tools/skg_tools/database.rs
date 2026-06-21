// ==========================================
// 🗄️ SKG DATABASE TOOLS — Native Skelegent Implementations
// ==========================================

use rusqlite::types::ValueRef;
use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── sqlite_query ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "sqlite_query",
    description = "Executes a raw SQL query against a specified SQLite database file. Returns a JSON string of the resulting rows."
)]
pub async fn sqlite_query(
    db_path: String,
    query: String,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let db_resolved = shellexpand::tilde(&db_path).to_string();

    let res = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        let conn = rusqlite::Connection::open(&db_resolved)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to open database: {}", e)))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| ToolError::ExecutionFailed(format!("WAL fail: {}", e)))?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")
            .map_err(|e| ToolError::ExecutionFailed(format!("WAL fail: {}", e)))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| ToolError::ExecutionFailed(format!("WAL fail: {}", e)))?;

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| ToolError::ExecutionFailed(format!("SQL prepare failed: {}", e)))?;

        let column_names: Vec<String> = stmt
            .column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        if column_names.is_empty() {
            let changed = stmt
                .execute([])
                .map_err(|e| ToolError::ExecutionFailed(format!("SQL execute failed: {}", e)))?;
            return Ok(format!(
                "Query executed successfully. {} rows affected.",
                changed
            ));
        }

        let mut rows = stmt
            .query([])
            .map_err(|e| ToolError::ExecutionFailed(format!("SQL query failed: {}", e)))?;
        let mut results = Vec::new();

        while let Some(row) = rows
            .next()
            .map_err(|e| ToolError::ExecutionFailed(format!("Row retrieval failed: {}", e)))?
        {
            let mut row_map = serde_json::Map::new();
            for (i, name) in column_names.iter().enumerate() {
                let val_ref = row
                    .get_ref(i)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Val ref failed: {}", e)))?;
                let val = match val_ref {
                    ValueRef::Null => serde_json::Value::Null,
                    ValueRef::Integer(v) => serde_json::json!(v),
                    ValueRef::Real(v) => serde_json::json!(v),
                    ValueRef::Text(t) => serde_json::json!(String::from_utf8_lossy(t)),
                    ValueRef::Blob(b) => serde_json::json!(format!("<Blob {} bytes>", b.len())),
                };
                row_map.insert(name.clone(), val);
            }
            results.push(serde_json::Value::Object(row_map));
        }

        serde_json::to_string_pretty(&results)
            .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization failed: {}", e)))
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task spawn failed: {}", e)))??;

    Ok(serde_json::Value::String(res))
}
