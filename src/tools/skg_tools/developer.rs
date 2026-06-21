// ==========================================
// 🛠️ SKG DEVELOPER TOOLS — Native Skelegent Implementations
// ==========================================

use super::execution::run_command;
use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── initialize_rust_project ───────────────────────────────────────────────────

#[skg_tool(
    name = "initialize_rust_project",
    description = "Creates a new Rust project using 'cargo new' and optionally adds dependencies. A one-turn high-level bootstrap tool."
)]
pub async fn initialize_rust_project(
    name: String,
    path: Option<String>,
    dependencies: Option<Vec<String>>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut working_dir = path.unwrap_or_else(|| ".".to_string());
    if working_dir.is_empty() {
        working_dir = ".".to_string();
    }
    let deps = dependencies.unwrap_or_default();

    // 1. Run cargo new
    let cargo_new_cmd = format!("cargo new {}", name);
    let res1_val = run_command(cargo_new_cmd, Some(working_dir.clone()), Some(60), ctx).await?;
    let res1 = res1_val.as_str().unwrap_or("");

    if res1.contains("error:") || res1.contains("Exit Status: exit status: 101") {
        return Ok(serde_json::Value::String(format!(
            "Failed to initialize project: {}",
            res1
        )));
    }

    // 2. Add dependencies
    let project_path = if working_dir == "." {
        name.clone()
    } else {
        format!("{}/{}", working_dir, name)
    };
    let mut dep_results = String::new();
    if !deps.is_empty() {
        let deps_str = deps.join(" ");
        let cargo_add_cmd = format!("cargo add {}", deps_str);
        let res2_val = run_command(cargo_add_cmd, Some(project_path), Some(120), ctx).await?;
        dep_results = res2_val.as_str().unwrap_or("").to_string();
    }

    let report = format!(
        "Successfully initialized Rust project '{}'.\n\nInitialization Output:\n{}\n\nDependency Update:\n{}",
        name, res1, dep_results
    );
    Ok(serde_json::Value::String(report))
}
