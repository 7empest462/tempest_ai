use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use arborium::tree_sitter::{Parser, Node, Language};

/// Detect the tree-sitter language from a file extension.
fn language_for_extension(ext: &str) -> Option<Language> {
    arborium::get_language(ext)
}

/// Recursively walk the AST and collect structural nodes (functions, structs, classes, etc.)
fn collect_symbols(node: Node, source: &[u8], depth: usize, symbols: &mut Vec<String>) {
    let kind = node.kind();
    
    // Collect meaningful structural nodes
    let is_structural = matches!(kind,
        "function_item" | "function_definition" | "function_declaration" |
        "struct_item" | "class_definition" | "class_declaration" |
        "impl_item" | "trait_item" | "enum_item" |
        "method_definition" | "arrow_function" |
        "mod_item" | "use_declaration" |
        "interface_declaration" | "type_alias_declaration" |
        "const_item" | "static_item" |
        "decorated_definition"
    );

    if is_structural {
        // Extract the name of the symbol
        let name = node.child_by_field_name("name")
            .map(|n| n.utf8_text(source).unwrap_or("?"))
            .unwrap_or_else(|| {
                // For impl blocks, try to get the type name
                node.child_by_field_name("type")
                    .map(|n| n.utf8_text(source).unwrap_or("?"))
                    .unwrap_or("(anonymous)")
            });

        let start = node.start_position();
        let end = node.end_position();
        let indent = "  ".repeat(depth);
        symbols.push(format!(
            "{}[L{}-L{}] {} `{}`",
            indent,
            start.row + 1,
            end.row + 1,
            kind,
            name
        ));
    }

    // Recurse into children
    let next_depth = if is_structural { depth + 1 } else { depth };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, next_depth, symbols);
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AstOutlineArgs {
    /// Path to the source file to analyze.
    pub path: String,
}

pub struct AstOutlineTool;

#[async_trait]
impl AgentTool for AstOutlineTool {
    fn name(&self) -> &'static str { "ast_outline" }
    fn description(&self) -> &'static str { 
        "Parse a source file using tree-sitter and return a structural outline (functions, structs, classes, impls) with line numbers. Supports Rust, Python, JavaScript, TypeScript. Use this to understand code structure before editing." 
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<AstOutlineArgs>();
        
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
        let typed_args: AstOutlineArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();

        let path = std::path::Path::new(&path_owned);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language = language_for_extension(ext)
            .ok_or_else(|| miette!("Unsupported file type: '.{}'. Supported: any language enabled in arborium config.", ext))?;

        let source = tokio::task::spawn_blocking({
            let p = path_owned.clone();
            move || std::fs::read_to_string(&p).map_err(|e| miette!("Failed to read file: {}", e))
        }).await.map_err(|e| miette!("Task join error: {}", e))??;

        let mut parser = Parser::new();
        parser.set_language(&language).map_err(|e| miette!("Parser init error: {}", e))?;
        
        let tree = parser.parse(&source, None)
            .ok_or_else(|| miette!("Failed to parse file: {}", path_owned))?;

        let mut symbols = Vec::new();
        collect_symbols(tree.root_node(), source.as_bytes(), 0, &mut symbols);

        if symbols.is_empty() {
            Ok(format!("📄 {} — No structural symbols found (file may be empty or contain only expressions).", path_owned))
        } else {
            let total_lines = source.lines().count();
            let header = format!(
                "📄 AST Outline: {} ({} lines, {} symbols)\n{}\n{}",
                path_owned,
                total_lines,
                symbols.len(),
                "─".repeat(50),
                symbols.join("\n")
            );
            Ok(header)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct AstEditArgs {
    /// Path to the source file.
    pub path: String,
    /// The name of the function/struct/class to find (e.g. "calculate_optimal_ctx").
    pub symbol_name: String,
    /// New content to replace the entire symbol body with.
    pub new_content: String,
}

pub struct AstEditTool;

#[async_trait]
impl AgentTool for AstEditTool {
    fn name(&self) -> &'static str { "ast_edit" }
    fn description(&self) -> &'static str { 
        "Replace an entire function, struct, or class by name using tree-sitter AST lookup. This is safer than line-based patch_file because it finds the symbol by name, not by brittle line numbers." 
    }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<AstEditArgs>();
        
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
        let typed_args: AstEditArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let target_name = typed_args.symbol_name;
        let new_content = typed_args.new_content;

        if new_content.contains("...existing code...") || new_content.contains("// unchanged") {
            return Err(miette!("Guardrail: Placeholder detected. You must provide the full symbol content."));
        }

        let path = std::path::Path::new(&path_owned);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language = language_for_extension(ext)
            .ok_or_else(|| miette!("Unsupported file type: '.{}'. Supported: any language enabled in arborium config.", ext))?;

        let source = std::fs::read_to_string(&path_owned)
            .map_err(|e| miette!("Failed to read file: {}", e))?;

        let mut parser = Parser::new();
        parser.set_language(&language).map_err(|e| miette!("Parser init error: {}", e))?;
        
        let tree = parser.parse(&source, None)
            .ok_or_else(|| miette!("Failed to parse file: {}", path_owned))?;

        // Find the target symbol by walking the AST
        fn find_symbol<'a>(node: Node<'a>, source: &'a [u8], target: &str) -> Option<(usize, usize)> {
            let kind = node.kind();
            let is_structural = matches!(kind,
                "function_item" | "function_definition" | "function_declaration" |
                "struct_item" | "class_definition" | "class_declaration" |
                "impl_item" | "trait_item" | "enum_item" |
                "method_definition" | "arrow_function" |
                "mod_item" | "interface_declaration" | "type_alias_declaration" |
                "const_item" | "static_item"
            );

            if is_structural {
                let name = node.child_by_field_name("name")
                    .or_else(|| node.child_by_field_name("type"))
                    .map(|n| n.utf8_text(source).unwrap_or(""));
                
                if name == Some(target) {
                    return Some((node.start_byte(), node.end_byte()));
                }
            }

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(result) = find_symbol(child, source, target) {
                    return Some(result);
                }
            }
            None
        }

        let (start_byte, end_byte) = find_symbol(tree.root_node(), source.as_bytes(), &target_name)
            .ok_or_else(|| miette!("Symbol '{}' not found in {}. Use `ast_outline` first to see available symbols.", target_name, path_owned))?;

        // Perform the replacement
        let mut result = String::with_capacity(source.len());
        result.push_str(&source[..start_byte]);
        result.push_str(&new_content);
        result.push_str(&source[end_byte..]);

        std::fs::write(&path_owned, &result)
            .map_err(|e| miette!("Failed to write file: {}", e))?;

        let old_lines = source[start_byte..end_byte].lines().count();
        let new_lines = new_content.lines().count();

        Ok(format!(
            "✅ AST Edit: Replaced symbol '{}' in {} ({} lines → {} lines)",
            target_name, path_owned, old_lines, new_lines
        ))
    }
}
