use serde_json::{json, Value};
use miette::{Result, IntoDiagnostic};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use super::execution::RunCommandTool;

#[derive(Deserialize, JsonSchema)]
pub struct InitializeRustProjectArgs {
    /// The name of the new project.
    pub name: String,
    /// Path where the project should be created (defaults to CWD).
    pub path: Option<String>,
    /// Optional list of dependencies to add (e.g., ["serde", "tokio"]).
    pub dependencies: Option<Vec<String>>,
}

pub struct InitializeRustProjectTool;

#[async_trait]
impl AgentTool for InitializeRustProjectTool {
    fn name(&self) -> &'static str { "initialize_rust_project" }
    fn description(&self) -> &'static str { "Creates a new Rust project using 'cargo new' and optionally adds dependencies. A one-turn high-level bootstrap tool." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<InitializeRustProjectArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: InitializeRustProjectArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let name = typed_args.name;
        let path = typed_args.path.unwrap_or_else(|| ".".to_string());
        let deps = typed_args.dependencies.unwrap_or_default();

        // 1. Run cargo new
        let cargo_new_cmd = format!("cargo new {}", name);
        let cargo_new_args = json!({ "command": cargo_new_cmd, "cwd": path, "timeout_seconds": 60 });
        let res1 = RunCommandTool.execute(&cargo_new_args, context.clone()).await?;

        if res1.contains("error:") || res1.contains("Exit Status: exit status: 101") {
            return Ok(format!("Failed to initialize project: {}", res1));
        }

        // 2. Add dependencies
        let project_path = if path == "." { name.clone() } else { format!("{}/{}", path, name) };
        let mut dep_results = String::new();
        if !deps.is_empty() {
            let deps_str = deps.join(" ");
            let cargo_add_cmd = format!("cargo add {}", deps_str);
            let cargo_add_args = json!({ "command": cargo_add_cmd, "cwd": project_path, "timeout_seconds": 120 });
            dep_results = RunCommandTool.execute(&cargo_add_args, context.clone()).await?;
        }

        Ok(format!("Successfully initialized Rust project '{}'.\n\nInitialization Output:\n{}\n\nDependency Update:\n{}", name, res1, dep_results))
    }
}
