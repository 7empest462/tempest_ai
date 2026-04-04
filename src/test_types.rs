use schemars::JsonSchema;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(JsonSchema)]
pub struct SystemInfoArgs {}

pub fn get_info() -> ToolInfo {
    let mut settings = schemars::gen::SchemaSettings::draft07();
    settings.inline_subschemas = true;
    let mut generator = settings.into_generator();
    let parameters = generator.into_root_schema_for::<SystemInfoArgs>();
    
    ToolInfo {
        tool_type: ToolType::Function,
        function: ToolFunctionInfo {
            name: "sys".to_string(),
            description: "desc".to_string(),
            parameters: schemars::schema::Schema::Object(parameters.schema),
        }
    }
}
