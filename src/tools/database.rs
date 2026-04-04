use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct SqliteQueryArgs {
    /// Absolute path (or ~/) to the .sqlite or .db file.
    pub db_path: String,
    /// The exact SQL query to execute (e.g., 'SELECT * FROM users LIMIT 5;').
    pub query: String,
}

pub struct SqliteQueryTool;

#[async_trait]
impl AgentTool for SqliteQueryTool {
    fn name(&self) -> &'static str { "sqlite_query" }
    fn description(&self) -> &'static str { "Executes a raw SQL query against a specified SQLite database file. Returns a JSON string of the resulting rows." }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<SqliteQueryArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: SqliteQueryArgs = serde_json::from_value(args.clone())?;
        let db_path = shellexpand::tilde(&typed_args.db_path).to_string();
        let query = typed_args.query;

        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let mut stmt = conn.prepare(&query)?;
        
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
