// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See the LICENSE file in the project root for full license text.

use serde_json::Value;
use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use crate::wasm_engine::WasmSandboxEngine;

#[derive(Deserialize, JsonSchema)]
pub struct WasmSafeCalculatorArgs {
    /// The left-hand side integer parameter.
    pub lh: i32,
    /// The right-hand side integer parameter.
    pub rh: i32,
    /// The operation to execute: "add", "sub", "mul", or "div".
    pub op: String,
}

pub struct WasmSafeCalculatorTool;

#[async_trait]
impl AgentTool for WasmSafeCalculatorTool {
    fn name(&self) -> &'static str { "wasm_safe_calc" }
    
    fn description(&self) -> &'static str { 
        "Executes a safe, resource-capped mathematical calculation inside an isolated, fuel-limited WASM sandbox container." 
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<WasmSafeCalculatorArgs>();
        
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
        let typed_args: WasmSafeCalculatorArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let op_lower = typed_args.op.to_lowercase();
        
        if op_lower != "add" && op_lower != "sub" && op_lower != "mul" && op_lower != "div" {
            return Err(miette!("Invalid operation '{}'. Supported: 'add', 'sub', 'mul', 'div'", typed_args.op));
        }
        
        if op_lower == "div" && typed_args.rh == 0 {
            return Err(miette!("WASM Execution Error: Division by zero is prohibited."));
        }
        
        let engine = WasmSandboxEngine::new()?;
        let res = engine.run_calculator(typed_args.lh, typed_args.rh, &op_lower)?;
        
        Ok(format!("SUCCESS: Sandboxed WASM operation '{}' ({} and {}) returned result: {}", op_lower, typed_args.lh, typed_args.rh, res))
    }
}
