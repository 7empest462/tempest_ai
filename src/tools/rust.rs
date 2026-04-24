use serde_json::Value;
use miette::{Result, IntoDiagnostic};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use std::process::Command;

#[derive(Deserialize, JsonSchema)]
pub struct CargoAddArgs {
    /// The name of the crate to add.
    pub crate_name: String,
    /// Optional version requirement (e.g., "1.0", "~2.1").
    pub version: Option<String>,
    /// Optional features to enable.
    pub features: Option<Vec<String>>,
    /// Whether to add as a dev-dependency.
    pub dev: Option<bool>,
    /// The directory where Cargo.toml is located. Defaults to current directory.
    pub cwd: Option<String>,
}

pub struct CargoAddTool;

#[async_trait]
impl AgentTool for CargoAddTool {
    fn name(&self) -> &'static str { "cargo_add" }
    fn description(&self) -> &'static str { "Helps with Rust and Cargo. Use this tool to add dependencies with 'cargo add'. ALWAYS use cargo_search first to verify the crate and get the correct version. Never guess crate versions." }
    fn is_modifying(&self) -> bool { true }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<CargoAddArgs>();
        
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
        let typed_args: CargoAddArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let mut cmd = Command::new("cargo");
        cmd.arg("add");
        
        if let Some(v) = typed_args.version {
            cmd.arg(format!("{}@{}", typed_args.crate_name, v));
        } else {
            cmd.arg(&typed_args.crate_name);
        }
        
        if let Some(features) = typed_args.features {
            if !features.is_empty() {
                cmd.arg("--features").arg(features.join(","));
            }
        }
        
        if typed_args.dev.unwrap_or(false) {
            cmd.arg("--dev");
        }

        if let Some(cwd) = typed_args.cwd {
            let path = std::path::Path::new(&cwd);
            if !path.exists() {
                return Err(miette::miette!("The directory '{}' does not exist. Please provide a valid path.", cwd));
            }
            cmd.current_dir(cwd);
        }

        let output = cmd.output().into_diagnostic()?;
        
        if output.status.success() {
            Ok(format!("Successfully added '{}' to dependencies.\n{}", 
                typed_args.crate_name, 
                String::from_utf8_lossy(&output.stderr)))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = if stderr.contains("could not be found in registry index") {
                format!("⚠️ SYSTEM DIRECTIVE: The version or crate name '{}' was not found. YOU JUST HALLUCINATED. Stop immediately. Use 'cargo_search' or 'search_web' to find the CORRECT name and version before retrying.", typed_args.crate_name)
            } else {
                format!("Failed to add dependency: {}", stderr)
            };
            Err(miette::miette!(error_msg))
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct CrateSearchArgs {
    /// The query string to search for on crates.io.
    pub query: String,
    /// Optional directory to run the search from.
    pub cwd: Option<String>,
}

pub struct CrateSearchTool;

#[async_trait]
impl AgentTool for CrateSearchTool {
    fn name(&self) -> &'static str { "cargo_search" }
    fn description(&self) -> &'static str { "Helps with Rust and Cargo. Use this tool to search for crates on crates.io, get the latest version of any crate. ALWAYS use this tool before suggesting any crate name or version in your response. Never guess crate versions." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<CrateSearchArgs>();
        
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
        let typed_args: CrateSearchArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let mut cmd = Command::new("cargo");
        cmd.arg("search")
            .arg(&typed_args.query)
            .arg("--limit")
            .arg("10");

        if let Some(cwd) = typed_args.cwd {
            let path = std::path::Path::new(&cwd);
            if !path.exists() {
                return Err(miette::miette!("The directory '{}' does not exist. Please provide a valid path.", cwd));
            }
            cmd.current_dir(cwd);
        }

        let output = cmd.output().into_diagnostic()?;
            
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if stdout.trim().is_empty() {
                Ok("No crates found matching your query. Check spelling or try a broader search. You may also use 'search_web' to find the crate on crates.io if cargo search fails.".to_string())
            } else {
                Ok(stdout)
            }
        } else {
            Err(miette::miette!("Cargo search failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }
}
