use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;

#[derive(JsonSchema)]
pub struct SystemInfoArgs {}

pub fn get_info() -> ToolInfo {
    let mut settings = schemars::generate::SchemaSettings::draft07();
    settings.inline_subschemas = true;
    let generator = settings.into_generator();
    let root = generator.into_root_schema_for::<SystemInfoArgs>();

    ToolInfo {
        tool_type: ToolType::Function,
        function: ToolFunctionInfo {
            name: "sys".to_string(),
            description: "desc".to_string(),
            parameters: root,
        },
    }
}
