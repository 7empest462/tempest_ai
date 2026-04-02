use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};

pub struct SqliteQueryTool;

#[async_trait]
impl AgentTool for SqliteQueryTool {
    fn name(&self) -> &'static str { "sqlite_query" }
    fn description(&self) -> &'static str { "Executes a raw SQL query against a specified SQLite database file. Returns a JSON string of the resulting rows." }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "db_path": { "type": "string", "description": "Absolute path (or ~/) to the .sqlite or .db file." },
                "query": { "type": "string", "description": "The exact SQL query to execute (e.g., 'SELECT * FROM users LIMIT 5;')." }
            },
            "required": ["db_path", "query"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let db_path_str = args.get("db_path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'db_path' argument"))?;
        let db_path = shellexpand::tilde(db_path_str).to_string();
        
        let query = args.get("query").and_then(|q| q.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;

        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let mut stmt = conn.prepare(query)?;
        
        let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

        if column_names.is_empty() {
            let changed = stmt.execute([])?;
            return Ok(format!("Query executed successfully. {} rows affected.", changed));
        }

        let mut rows = stmt.query([])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let mut row_map = serde_json::Map::new();
            for (i, name) in column_names.iter().enumerate() {
                let val_ref = row.get_ref(i)?;
                use rusqlite::types::ValueRef;
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

        Ok(serde_json::to_string_pretty(&results)?)
    }
}
