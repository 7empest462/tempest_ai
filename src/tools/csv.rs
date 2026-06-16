use super::{AgentTool, ToolContext};
use async_trait::async_trait;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::path::Path;

#[derive(Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CsvAction {
    InspectHeaders,
    GetRows,
    FilterByColumn,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct QueryCsvArgs {
    /// Absolute path (or ~/) to the CSV file.
    pub csv_path: String,
    /// The action to perform:
    /// - `inspect_headers`: Returns the headers/columns of the CSV and their index.
    /// - `get_rows`: Returns a range of rows (controlled by `limit` and `offset`).
    /// - `filter_by_column`: Filters rows by matching a column value (case-insensitive substring match).
    pub action: CsvAction,
    /// Optional column name for filtering (required if action is `filter_by_column`).
    pub filter_column: Option<String>,
    /// Optional pattern/value to filter by (required if action is `filter_by_column`).
    pub filter_value: Option<String>,
    /// Optional maximum number of rows to return (default is 100, max is 500).
    pub limit: Option<usize>,
    /// Optional offset/starting row for retrieval (default is 0).
    pub offset: Option<usize>,
    /// Optional format: `markdown` (default) or `json`.
    pub output_format: Option<String>,
}

pub struct QueryCsvTool;

#[async_trait]
impl AgentTool for QueryCsvTool {
    fn name(&self) -> &'static str {
        "query_csv"
    }
    fn description(&self) -> &'static str {
        "Parses and queries a local CSV file. Supports inspecting headers, pagination (limit/offset), and filtering by column."
    }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<QueryCsvArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: QueryCsvArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let csv_path = shellexpand::tilde(&typed_args.csv_path).to_string();

        // Use tokio::task::spawn_blocking because CSV parsing and disk IO can block
        tokio::task::spawn_blocking(move || {
            if !Path::new(&csv_path).exists() {
                return Err(miette!("CSV file not found at: {}", csv_path));
            }

            let file = File::open(&csv_path).into_diagnostic()?;
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(file);

            let headers = rdr.headers().into_diagnostic()?.clone();
            let header_list: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

            match typed_args.action {
                CsvAction::InspectHeaders => {
                    let is_json = typed_args.output_format.as_deref() == Some("json");
                    if is_json {
                        let json_headers = serde_json::json!({
                            "headers": header_list,
                            "column_count": header_list.len(),
                        });
                        Ok(serde_json::to_string_pretty(&json_headers).into_diagnostic()?)
                    } else {
                        let mut output = format!("CSV Headers (Total columns: {}):\n\n", header_list.len());
                        for (i, h) in header_list.iter().enumerate() {
                            output.push_str(&format!("- Index {}: `{}`\n", i, h));
                        }
                        Ok(output)
                    }
                }
                CsvAction::GetRows => {
                    let limit = typed_args.limit.unwrap_or(100).min(500);
                    let offset = typed_args.offset.unwrap_or(0);
                    let is_json = typed_args.output_format.as_deref() == Some("json");

                    let mut records = Vec::new();
                    let mut total_rows = 0;

                    for result in rdr.records() {
                        let record = result.into_diagnostic()?;
                        if total_rows >= offset && total_rows < offset + limit {
                            records.push(record);
                        }
                        total_rows += 1;
                    }

                    if is_json {
                        let json_rows = format_rows_as_json(&header_list, &records);
                        let output_val = serde_json::json!({
                            "total_rows": total_rows,
                            "offset": offset,
                            "limit": limit,
                            "rows": json_rows,
                        });
                        Ok(serde_json::to_string_pretty(&output_val).into_diagnostic()?)
                    } else {
                        let table = format_rows_as_markdown(&header_list, &records);
                        let output = format!(
                            "Showing rows {}-{} of {} total rows:\n\n{}",
                            offset,
                            offset + records.len(),
                            total_rows,
                            table
                        );
                        Ok(output)
                    }
                }
                CsvAction::FilterByColumn => {
                    let filter_col = typed_args.filter_column.ok_or_else(|| {
                        miette!("`filter_column` is required when action is `filter_by_column`")
                    })?;
                    let filter_val = typed_args.filter_value.ok_or_else(|| {
                        miette!("`filter_value` is required when action is `filter_by_column`")
                    })?;

                    // Find index of the filter column
                    let col_idx = header_list.iter().position(|h| h.eq_ignore_ascii_case(&filter_col))
                        .ok_or_else(|| {
                            miette!(
                                "Column `{}` not found in CSV. Available columns: {:?}",
                                filter_col,
                                header_list
                            )
                        })?;

                    let limit = typed_args.limit.unwrap_or(100).min(500);
                    let offset = typed_args.offset.unwrap_or(0);
                    let is_json = typed_args.output_format.as_deref() == Some("json");
                    let filter_val_lower = filter_val.to_lowercase();

                    let mut matched_records = Vec::new();
                    let mut matched_count = 0;

                    for result in rdr.records() {
                        let record = result.into_diagnostic()?;
                        if let Some(val) = record.get(col_idx)
                            && val.to_lowercase().contains(&filter_val_lower) {
                                if matched_count >= offset && matched_count < offset + limit {
                                    matched_records.push(record);
                                }
                                matched_count += 1;
                            }
                    }

                    if is_json {
                        let json_rows = format_rows_as_json(&header_list, &matched_records);
                        let output_val = serde_json::json!({
                            "matched_count": matched_count,
                            "offset": offset,
                            "limit": limit,
                            "rows": json_rows,
                        });
                        Ok(serde_json::to_string_pretty(&output_val).into_diagnostic()?)
                    } else {
                        let table = format_rows_as_markdown(&header_list, &matched_records);
                        let output = format!(
                            "Showing matching rows {}-{} of {} total matches (Filter: `{}` contains `{}`):\n\n{}",
                            offset,
                            offset + matched_records.len(),
                            matched_count,
                            filter_col,
                            filter_val,
                            table
                        );
                        Ok(output)
                    }
                }
            }
        }).await.map_err(|e| miette!("CSV task failed: {}", e))?
    }
}

fn format_rows_as_json(
    headers: &[String],
    records: &[csv::StringRecord],
) -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    for rec in records {
        let mut row_map = serde_json::Map::new();
        for (i, h) in headers.iter().enumerate() {
            let val = rec.get(i).unwrap_or("").to_string();
            row_map.insert(h.clone(), serde_json::Value::String(val));
        }
        rows.push(serde_json::Value::Object(row_map));
    }
    rows
}

fn format_rows_as_markdown(headers: &[String], records: &[csv::StringRecord]) -> String {
    if records.is_empty() {
        return "No rows to display.".to_string();
    }
    let mut table = String::new();

    // Table Header
    table.push_str("| ");
    for h in headers {
        table.push_str(h);
        table.push_str(" | ");
    }
    table.push('\n');

    // Separator
    table.push_str("| ");
    for _ in headers {
        table.push_str("--- | ");
    }
    table.push('\n');

    // Table rows
    for rec in records {
        table.push_str("| ");
        for i in 0..headers.len() {
            let val = rec.get(i).unwrap_or("").replace('|', "\\|");
            table.push_str(&val);
            table.push_str(" | ");
        }
        table.push('\n');
    }

    table
}
