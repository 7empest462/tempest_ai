// ==========================================
// 📊 SKG CSV QUERY TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::fs::File;
use std::path::Path;

#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CsvAction {
    InspectHeaders,
    GetRows,
    FilterByColumn,
}

// ── query_csv ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "query_csv",
    description = "Parses and queries a local CSV file. Supports inspecting headers, pagination (limit/offset), and filtering by column."
)]
#[allow(clippy::too_many_arguments)]
pub async fn query_csv(
    csv_path: String,
    action: CsvAction,
    filter_column: Option<String>,
    filter_value: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    output_format: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let resolved_path = shellexpand::tilde(&csv_path).to_string();

    let res = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
        if !Path::new(&resolved_path).exists() {
            return Err(ToolError::ExecutionFailed(format!("CSV file not found at: {}", resolved_path)));
        }

        let file = File::open(&resolved_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to open CSV file: {}", e)))?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(file);

        let headers = rdr.headers()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read CSV headers: {}", e)))?
            .clone();
        let header_list: Vec<String> = headers.iter().map(|s| s.to_string()).collect();

        match action {
            CsvAction::InspectHeaders => {
                let is_json = output_format.as_deref() == Some("json");
                if is_json {
                    let json_headers = serde_json::json!({
                        "headers": header_list,
                        "column_count": header_list.len(),
                    });
                    serde_json::to_string_pretty(&json_headers)
                        .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization failed: {}", e)))
                } else {
                    let mut output = format!("CSV Headers (Total columns: {}):\n\n", header_list.len());
                    for (i, h) in header_list.iter().enumerate() {
                        output.push_str(&format!("- Index {}: `{}`\n", i, h));
                    }
                    Ok(output)
                }
            }
            CsvAction::GetRows => {
                let limit_val = limit.unwrap_or(100).min(500);
                let offset_val = offset.unwrap_or(0);
                let is_json = output_format.as_deref() == Some("json");

                let mut records = Vec::new();
                let mut total_rows = 0;

                for result in rdr.records() {
                    let record = result
                        .map_err(|e| ToolError::ExecutionFailed(format!("Error parsing row: {}", e)))?;
                    if total_rows >= offset_val && total_rows < offset_val + limit_val {
                        records.push(record);
                    }
                    total_rows += 1;
                }

                if is_json {
                    let json_rows = format_rows_as_json(&header_list, &records);
                    let output_val = serde_json::json!({
                        "total_rows": total_rows,
                        "offset": offset_val,
                        "limit": limit_val,
                        "rows": json_rows,
                    });
                    serde_json::to_string_pretty(&output_val)
                        .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization failed: {}", e)))
                } else {
                    let table = format_rows_as_markdown(&header_list, &records);
                    let output = format!(
                        "Showing rows {}-{} of {} total rows:\n\n{}",
                        offset_val,
                        offset_val + records.len(),
                        total_rows,
                        table
                    );
                    Ok(output)
                }
            }
            CsvAction::FilterByColumn => {
                let filter_col = filter_column.ok_or_else(|| {
                    ToolError::ExecutionFailed("`filter_column` is required when action is `filter_by_column`".to_string())
                })?;
                let filter_val = filter_value.ok_or_else(|| {
                    ToolError::ExecutionFailed("`filter_value` is required when action is `filter_by_column`".to_string())
                })?;

                let col_idx = header_list.iter().position(|h| h.eq_ignore_ascii_case(&filter_col))
                    .ok_or_else(|| {
                        ToolError::ExecutionFailed(format!(
                            "Column `{}` not found in CSV. Available columns: {:?}",
                            filter_col,
                            header_list
                        ))
                    })?;

                let limit_val = limit.unwrap_or(100).min(500);
                let offset_val = offset.unwrap_or(0);
                let is_json = output_format.as_deref() == Some("json");
                let filter_val_lower = filter_val.to_lowercase();

                let mut matched_records = Vec::new();
                let mut matched_count = 0;

                for result in rdr.records() {
                    let record = result
                        .map_err(|e| ToolError::ExecutionFailed(format!("Error parsing row: {}", e)))?;
                    if let Some(val) = record.get(col_idx)
                        && val.to_lowercase().contains(&filter_val_lower) {
                            if matched_count >= offset_val && matched_count < offset_val + limit_val {
                                matched_records.push(record);
                            }
                            matched_count += 1;
                        }
                }

                if is_json {
                    let json_rows = format_rows_as_json(&header_list, &matched_records);
                    let output_val = serde_json::json!({
                        "matched_count": matched_count,
                        "offset": offset_val,
                        "limit": limit_val,
                        "rows": json_rows,
                    });
                    serde_json::to_string_pretty(&output_val)
                        .map_err(|e| ToolError::ExecutionFailed(format!("JSON serialization failed: {}", e)))
                } else {
                    let table = format_rows_as_markdown(&header_list, &matched_records);
                    let output = format!(
                        "Showing matching rows {}-{} of {} total matches (Filter: `{}` contains `{}`):\n\n{}",
                        offset_val,
                        offset_val + matched_records.len(),
                        matched_count,
                        filter_col,
                        filter_val,
                        table
                    );
                    Ok(output)
                }
            }
        }
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task execution failed: {}", e)))??;

    Ok(serde_json::Value::String(res))
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
