use miette::{Result, IntoDiagnostic};
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use rusqlite::types::ValueRef;

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
        let payload = settings.into_generator().into_root_schema_for::<SqliteQueryArgs>();
        
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
        let typed_args: SqliteQueryArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let db_path = shellexpand::tilde(&typed_args.db_path).to_string();
        let query = typed_args.query;

        let conn = rusqlite::Connection::open(&db_path).into_diagnostic()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;").into_diagnostic()?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;").into_diagnostic()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;").into_diagnostic()?;
        let mut stmt = conn.prepare(&query).into_diagnostic()?;
        
        let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

        if column_names.is_empty() {
            let changed = stmt.execute([]).into_diagnostic()?;
            return Ok(format!("Query executed successfully. {} rows affected.", changed));
        }

        let mut rows = stmt.query([]).into_diagnostic()?;
        let mut results = Vec::new();

        while let Some(row) = rows.next().into_diagnostic()? {
            let mut row_map = serde_json::Map::new();
            for (i, name) in column_names.iter().enumerate() {
                let val_ref = row.get_ref(i).into_diagnostic()?;
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

        serde_json::to_string_pretty(&results).into_diagnostic()
    }
}
